use anyhow::Result;
use std::fs;
use std::path::Path;

use crate::task::TaskResult;

/// Generate an HTML dashboard report
pub fn generate_report(
    project_name: &str,
    results: &[TaskResult],
    output_path: &Path,
) -> Result<()> {
    let html = build_html(project_name, results);
    fs::write(output_path, html)?;
    Ok(())
}

fn build_html(project_name: &str, results: &[TaskResult]) -> String {
    let total = results.len();
    let passed = results.iter().filter(|r| r.success).count();
    let failed = total - passed;
    let total_time: u128 = results.iter().map(|r| r.duration_ms).sum();
    let pass_rate = if total > 0 {
        (passed as f64 / total as f64) * 100.0
    } else {
        0.0
    };

    let timestamp = chrono_lite_now();

    // Collect unique categories
    let categories: Vec<String> = {
        let mut cats: Vec<String> = results
            .iter()
            .filter_map(|r| r.category.clone())
            .collect();
        cats.sort();
        cats.dedup();
        cats
    };

    // Build task data for JavaScript
    let task_data_js: String = results
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let cat = r.category.as_deref().unwrap_or("other");
            format!(
                "{{id:{},name:'{}',success:{},duration:{},category:'{}'}}",
                i, r.name, r.success, r.duration_ms, cat
            )
        })
        .collect::<Vec<_>>()
        .join(",");

    // Build category filter buttons
    let category_buttons: String = categories
        .iter()
        .map(|cat| {
            let count = results.iter().filter(|r| r.category.as_deref() == Some(cat)).count();
            format!(
                r#"<button class="filter-btn filter-cat" onclick="setCategoryFilter('{}', this)">{} ({})</button>"#,
                cat, cat.to_uppercase(), count
            )
        })
        .collect();

    // Build task cards (all hidden by default, shown by filter)
    let task_cards: String = results
        .iter()
        .enumerate()
        .map(|(i, r)| build_task_card(i, r))
        .collect();

    format!(
        r##"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{project_name} - Runx Dashboard</title>
    <script src="https://cdn.jsdelivr.net/npm/echarts@5.4.3/dist/echarts.min.js"></script>
    <style>
        * {{ margin: 0; padding: 0; box-sizing: border-box; }}
        body {{
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            background: #0f0f1a;
            color: #eee;
            min-height: 100vh;
        }}
        .header {{
            background: linear-gradient(135deg, #1a1a2e 0%, #16213e 100%);
            padding: 20px 30px;
            border-bottom: 1px solid #2d2d44;
            display: flex;
            justify-content: space-between;
            align-items: center;
            flex-wrap: wrap;
            gap: 15px;
        }}
        .header-left h1 {{
            color: #00d4ff;
            font-size: 28px;
            margin-bottom: 5px;
        }}
        .header-left p {{ color: #888; font-size: 14px; }}
        .summary {{
            display: flex;
            gap: 20px;
            padding: 20px 30px;
            background: #1a1a2e;
            border-bottom: 1px solid #2d2d44;
            flex-wrap: wrap;
        }}
        .summary-card {{
            background: #16213e;
            padding: 15px 25px;
            border-radius: 10px;
            text-align: center;
            min-width: 120px;
        }}
        .summary-card.success {{ border-left: 4px solid #26a69a; }}
        .summary-card.failure {{ border-left: 4px solid #ef5350; }}
        .summary-card.time {{ border-left: 4px solid #00d4ff; }}
        .summary-card.rate {{ border-left: 4px solid #ffd700; }}
        .summary-value {{ font-size: 28px; font-weight: bold; }}
        .summary-value.green {{ color: #26a69a; }}
        .summary-value.red {{ color: #ef5350; }}
        .summary-value.blue {{ color: #00d4ff; }}
        .summary-value.gold {{ color: #ffd700; }}
        .summary-label {{ font-size: 12px; color: #888; margin-top: 5px; }}

        /* Filter Bar */
        .filter-bar {{
            padding: 15px 30px;
            background: #16213e;
            border-bottom: 1px solid #2d2d44;
            display: flex;
            gap: 15px;
            flex-wrap: wrap;
            align-items: center;
        }}
        .search-box {{
            flex: 1;
            min-width: 200px;
            position: relative;
        }}
        .search-box input {{
            width: 100%;
            padding: 10px 15px 10px 40px;
            background: #1a1a2e;
            border: 1px solid #2d2d44;
            border-radius: 8px;
            color: #fff;
            font-size: 14px;
        }}
        .search-box input:focus {{
            outline: none;
            border-color: #00d4ff;
        }}
        .search-box::before {{
            content: '🔍';
            position: absolute;
            left: 12px;
            top: 50%;
            transform: translateY(-50%);
            font-size: 14px;
        }}
        .filter-btn {{
            padding: 10px 20px;
            background: #1a1a2e;
            border: 1px solid #2d2d44;
            color: #aaa;
            cursor: pointer;
            border-radius: 8px;
            font-size: 13px;
            transition: all 0.2s;
        }}
        .filter-btn:hover {{ background: #1f2b47; color: #fff; }}
        .filter-btn.active {{
            background: #00d4ff;
            color: #000;
            border-color: #00d4ff;
        }}
        .filter-btn.filter-pass.active {{ background: #26a69a; }}
        .filter-btn.filter-fail.active {{ background: #ef5350; }}
        .filter-btn.filter-cat {{ border-left: 3px solid #9c27b0; }}
        .filter-btn.filter-cat.active {{ background: #9c27b0; }}

        /* Main Content */
        .main-content {{
            display: flex;
            min-height: calc(100vh - 250px);
        }}
        .sidebar {{
            width: 280px;
            background: #1a1a2e;
            border-right: 1px solid #2d2d44;
            overflow-y: auto;
            max-height: calc(100vh - 250px);
        }}
        .sidebar-item {{
            padding: 12px 20px;
            border-bottom: 1px solid #2d2d44;
            cursor: pointer;
            transition: all 0.2s;
            display: flex;
            justify-content: space-between;
            align-items: center;
        }}
        .sidebar-item:hover {{ background: #16213e; }}
        .sidebar-item.active {{ background: #16213e; border-left: 3px solid #00d4ff; }}
        .sidebar-item.hidden {{ display: none; }}
        .sidebar-item .name {{ font-size: 14px; }}
        .sidebar-item .status {{
            padding: 3px 10px;
            border-radius: 12px;
            font-size: 11px;
            font-weight: bold;
        }}
        .sidebar-item .status.pass {{ background: #1b4332; color: #26a69a; }}
        .sidebar-item .status.fail {{ background: #4a1c1c; color: #ef5350; }}
        .sidebar-item .duration {{ font-size: 11px; color: #666; }}
        .cat-badge {{
            display: inline-block;
            padding: 2px 8px;
            border-radius: 10px;
            font-size: 10px;
            background: #2d2d44;
            color: #9c27b0;
            margin-left: 8px;
            text-transform: uppercase;
        }}

        .detail-panel {{
            flex: 1;
            padding: 30px;
            overflow-y: auto;
        }}
        .detail-panel.empty {{
            display: flex;
            align-items: center;
            justify-content: center;
            color: #666;
            font-size: 16px;
        }}

        /* Charts */
        .charts-grid {{
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(400px, 1fr));
            gap: 20px;
            margin-bottom: 30px;
        }}
        .chart-card {{
            background: #1a1a2e;
            border-radius: 12px;
            padding: 20px;
            border: 1px solid #2d2d44;
        }}
        .chart-card h3 {{ color: #fff; font-size: 16px; margin-bottom: 15px; }}
        .chart-container {{ height: 300px; }}

        /* Task Detail Card */
        .task-detail {{
            display: none;
        }}
        .task-detail.active {{ display: block; }}
        .task-header {{
            display: flex;
            justify-content: space-between;
            align-items: center;
            margin-bottom: 20px;
            padding-bottom: 15px;
            border-bottom: 1px solid #2d2d44;
        }}
        .task-title {{ font-size: 24px; color: #00d4ff; }}
        .task-badge {{
            padding: 8px 20px;
            border-radius: 20px;
            font-size: 14px;
            font-weight: bold;
        }}
        .task-badge.pass {{ background: #1b4332; color: #26a69a; }}
        .task-badge.fail {{ background: #4a1c1c; color: #ef5350; }}
        .task-stats {{
            display: flex;
            gap: 20px;
            margin-bottom: 20px;
            flex-wrap: wrap;
        }}
        .stat-box {{
            background: #16213e;
            padding: 15px 25px;
            border-radius: 10px;
            text-align: center;
        }}
        .stat-value {{ font-size: 24px; font-weight: bold; color: #00d4ff; }}
        .stat-label {{ font-size: 11px; color: #888; margin-top: 5px; }}

        /* Overview */
        #overview {{ display: block; }}
        .section-title {{
            color: #00d4ff;
            font-size: 22px;
            margin-bottom: 20px;
        }}
        .matrix-table {{
            width: 100%;
            border-collapse: collapse;
        }}
        .matrix-table th, .matrix-table td {{
            padding: 12px;
            text-align: left;
            border: 1px solid #2d2d44;
        }}
        .matrix-table th {{ background: #16213e; color: #00d4ff; }}
        .matrix-table tr:nth-child(even) {{ background: rgba(22, 33, 62, 0.5); }}
        .matrix-table tr:hover {{ background: #1f2b47; cursor: pointer; }}
        .pass {{ color: #26a69a; font-weight: bold; }}
        .fail {{ color: #ef5350; font-weight: bold; }}

        /* Empty state */
        .no-results {{
            text-align: center;
            padding: 40px;
            color: #666;
        }}
    </style>
</head>
<body>
    <div class="header">
        <div class="header-left">
            <h1>🚀 {project_name}</h1>
            <p>Runx Test Dashboard - {timestamp}</p>
        </div>
    </div>

    <div class="summary">
        <div class="summary-card success">
            <div class="summary-value green">{passed}</div>
            <div class="summary-label">Passed</div>
        </div>
        <div class="summary-card failure">
            <div class="summary-value red">{failed}</div>
            <div class="summary-label">Failed</div>
        </div>
        <div class="summary-card time">
            <div class="summary-value blue">{total_time}</div>
            <div class="summary-label">Total (ms)</div>
        </div>
        <div class="summary-card rate">
            <div class="summary-value gold">{pass_rate:.1}%</div>
            <div class="summary-label">Pass Rate</div>
        </div>
    </div>

    <div class="filter-bar">
        <div class="search-box">
            <input type="text" id="searchInput" placeholder="Search tasks..." oninput="filterTasks()">
        </div>
        <button class="filter-btn active" onclick="setFilter('all', this)">All ({total})</button>
        <button class="filter-btn filter-pass" onclick="setFilter('pass', this)">Passed ({passed})</button>
        <button class="filter-btn filter-fail" onclick="setFilter('fail', this)">Failed ({failed})</button>
        <span style="color:#444; margin: 0 10px;">|</span>
        <button class="filter-btn filter-cat active" onclick="setCategoryFilter('all', this)">All Types</button>
        {category_buttons}
    </div>

    <div class="main-content">
        <div class="sidebar" id="taskList">
            <div class="sidebar-item active" onclick="showOverview()">
                <span class="name">📊 Overview</span>
            </div>
            <!-- Task items will be generated by JS -->
        </div>

        <div class="detail-panel" id="detailPanel">
            <div id="overview">
                <h2 class="section-title">Results Overview</h2>
                <div class="charts-grid">
                    <div class="chart-card">
                        <h3>Task Duration Timeline</h3>
                        <div id="timeline-chart" class="chart-container"></div>
                    </div>
                    <div class="chart-card">
                        <h3>Pass/Fail Distribution</h3>
                        <div id="pie-chart" class="chart-container"></div>
                    </div>
                </div>

                <h3 class="section-title" style="margin-top: 30px;">All Tests</h3>
                <table class="matrix-table" id="resultsTable">
                    <thead>
                        <tr>
                            <th>Task</th>
                            <th>Status</th>
                            <th>Category</th>
                            <th>Duration</th>
                        </tr>
                    </thead>
                    <tbody id="tableBody"></tbody>
                </table>
            </div>

            {task_cards}
        </div>
    </div>

    <script>
        const tasks = [{task_data_js}];
        let currentFilter = 'all';
        let currentCategory = 'all';
        let currentSearch = '';

        // Initialize sidebar and table
        function initUI() {{
            const sidebar = document.getElementById('taskList');
            const tbody = document.getElementById('tableBody');

            tasks.forEach((task, i) => {{
                // Sidebar item
                const item = document.createElement('div');
                item.className = 'sidebar-item';
                item.dataset.id = i;
                item.dataset.success = task.success;
                const catBadge = task.category !== 'other' ? `<span class="cat-badge">${{task.category}}</span>` : '';
                item.innerHTML = `
                    <div>
                        <div class="name">${{task.name}} ${{catBadge}}</div>
                        <div class="duration">${{task.duration}}ms</div>
                    </div>
                    <span class="status ${{task.success ? 'pass' : 'fail'}}">${{task.success ? 'PASS' : 'FAIL'}}</span>
                `;
                item.onclick = () => showTask(i);
                sidebar.appendChild(item);

                // Table row
                const row = document.createElement('tr');
                row.dataset.id = i;
                row.dataset.success = task.success;
                row.innerHTML = `
                    <td>${{task.name}} ${{catBadge}}</td>
                    <td class="${{task.success ? 'pass' : 'fail'}}">${{task.success ? '✓ Passed' : '✗ Failed'}}</td>
                    <td>${{task.category}}</td>
                    <td>${{task.duration}}ms</td>
                `;
                row.onclick = () => showTask(i);
                tbody.appendChild(row);
            }});
        }}

        function setFilter(filter, btn) {{
            currentFilter = filter;
            document.querySelectorAll('.filter-btn:not(.filter-cat)').forEach(b => b.classList.remove('active'));
            btn.classList.add('active');
            filterTasks();
        }}

        function setCategoryFilter(category, btn) {{
            currentCategory = category;
            document.querySelectorAll('.filter-btn.filter-cat').forEach(b => b.classList.remove('active'));
            btn.classList.add('active');
            filterTasks();
        }}

        function filterTasks() {{
            currentSearch = document.getElementById('searchInput').value.toLowerCase();

            document.querySelectorAll('.sidebar-item[data-id]').forEach(item => {{
                const id = parseInt(item.dataset.id);
                const task = tasks[id];
                const matchesSearch = task.name.toLowerCase().includes(currentSearch);
                const matchesFilter = currentFilter === 'all' ||
                    (currentFilter === 'pass' && task.success) ||
                    (currentFilter === 'fail' && !task.success);
                const matchesCategory = currentCategory === 'all' || task.category === currentCategory;

                item.classList.toggle('hidden', !(matchesSearch && matchesFilter && matchesCategory));
            }});

            document.querySelectorAll('#tableBody tr').forEach(row => {{
                const id = parseInt(row.dataset.id);
                const task = tasks[id];
                const matchesSearch = task.name.toLowerCase().includes(currentSearch);
                const matchesFilter = currentFilter === 'all' ||
                    (currentFilter === 'pass' && task.success) ||
                    (currentFilter === 'fail' && !task.success);
                const matchesCategory = currentCategory === 'all' || task.category === currentCategory;

                row.style.display = (matchesSearch && matchesFilter && matchesCategory) ? '' : 'none';
            }});
        }}

        function showOverview() {{
            document.querySelectorAll('.sidebar-item').forEach(i => i.classList.remove('active'));
            document.querySelector('.sidebar-item').classList.add('active');
            document.getElementById('overview').style.display = 'block';
            document.querySelectorAll('.task-detail').forEach(d => d.classList.remove('active'));
        }}

        function showTask(id) {{
            document.querySelectorAll('.sidebar-item').forEach(i => i.classList.remove('active'));
            document.querySelector(`.sidebar-item[data-id="${{id}}"]`).classList.add('active');
            document.getElementById('overview').style.display = 'none';
            document.querySelectorAll('.task-detail').forEach(d => d.classList.remove('active'));
            document.getElementById('task_' + id).classList.add('active');
        }}

        // Initialize charts
        function initCharts() {{
            // Timeline chart
            const timelineChart = echarts.init(document.getElementById('timeline-chart'), 'dark');
            timelineChart.setOption({{
                backgroundColor: 'transparent',
                tooltip: {{ trigger: 'axis', formatter: '{{b}}: {{c}}ms' }},
                grid: {{ left: '5%', right: '5%', bottom: '20%', top: '10%', containLabel: true }},
                xAxis: {{
                    type: 'category',
                    data: tasks.map(t => t.name),
                    axisLabel: {{ rotate: 45, color: '#888', fontSize: 10 }}
                }},
                yAxis: {{
                    type: 'value',
                    name: 'ms',
                    axisLabel: {{ color: '#888' }}
                }},
                series: [{{
                    type: 'bar',
                    data: tasks.map(t => ({{
                        value: t.duration,
                        itemStyle: {{ color: t.success ? '#26a69a' : '#ef5350' }}
                    }})),
                    barWidth: '60%'
                }}]
            }});

            // Pie chart
            const pieChart = echarts.init(document.getElementById('pie-chart'), 'dark');
            pieChart.setOption({{
                backgroundColor: 'transparent',
                tooltip: {{ trigger: 'item' }},
                legend: {{ bottom: '5%', textStyle: {{ color: '#888' }} }},
                series: [{{
                    type: 'pie',
                    radius: ['40%', '70%'],
                    center: ['50%', '45%'],
                    data: [
                        {{ value: {passed}, name: 'Passed', itemStyle: {{ color: '#26a69a' }} }},
                        {{ value: {failed}, name: 'Failed', itemStyle: {{ color: '#ef5350' }} }}
                    ],
                    label: {{ color: '#fff', formatter: '{{b}}: {{c}}' }},
                    emphasis: {{ itemStyle: {{ shadowBlur: 10, shadowColor: 'rgba(0,0,0,0.5)' }} }}
                }}]
            }});

            window.addEventListener('resize', () => {{
                timelineChart.resize();
                pieChart.resize();
            }});
        }}

        // Initialize
        initUI();
        initCharts();
    </script>
</body>
</html>"##,
        project_name = project_name,
        timestamp = timestamp,
        total = total,
        passed = passed,
        failed = failed,
        total_time = total_time,
        pass_rate = pass_rate,
        task_data_js = task_data_js,
        task_cards = task_cards,
        category_buttons = category_buttons,
    )
}

fn build_task_card(index: usize, result: &TaskResult) -> String {
    let status_class = if result.success { "pass" } else { "fail" };
    let status_text = if result.success { "PASSED" } else { "FAILED" };

    format!(
        r##"<div id="task_{index}" class="task-detail">
        <div class="task-header">
            <span class="task-title">{name}</span>
            <span class="task-badge {status_class}">{status_text}</span>
        </div>
        <div class="task-stats">
            <div class="stat-box">
                <div class="stat-value">{duration}</div>
                <div class="stat-label">Duration (ms)</div>
            </div>
            <div class="stat-box">
                <div class="stat-value" style="color: {color}">{status_icon}</div>
                <div class="stat-label">Status</div>
            </div>
        </div>
        <div class="chart-card">
            <h3>Execution Timeline</h3>
            <div id="task_chart_{index}" class="chart-container"></div>
        </div>
        <script>
            (function() {{
                const chart = echarts.init(document.getElementById('task_chart_{index}'), 'dark');
                chart.setOption({{
                    backgroundColor: 'transparent',
                    tooltip: {{}},
                    xAxis: {{ type: 'category', data: ['Start', 'Running', 'End'] }},
                    yAxis: {{ type: 'value', show: false }},
                    series: [{{
                        type: 'line',
                        smooth: true,
                        areaStyle: {{ color: '{color}', opacity: 0.3 }},
                        lineStyle: {{ color: '{color}' }},
                        itemStyle: {{ color: '{color}' }},
                        data: [0, {duration}, {duration}]
                    }}]
                }});
                window.addEventListener('resize', () => chart.resize());
            }})();
        </script>
    </div>"##,
        index = index,
        name = result.name,
        status_class = status_class,
        status_text = status_text,
        duration = result.duration_ms,
        color = if result.success { "#26a69a" } else { "#ef5350" },
        status_icon = if result.success { "✓" } else { "✗" },
    )
}

fn chrono_lite_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = duration.as_secs();

    let days_since_epoch = secs / 86400;
    let years = 1970 + (days_since_epoch / 365);
    let remaining_days = days_since_epoch % 365;
    let month = remaining_days / 30 + 1;
    let day = remaining_days % 30 + 1;

    let time_of_day = secs % 86400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;

    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        years, month, day, hours, minutes, seconds
    )
}
