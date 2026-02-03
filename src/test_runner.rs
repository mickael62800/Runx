//! Test runner module
//!
//! Executes Rust tests with streaming output and real-time status updates.

#![allow(dead_code)]

use anyhow::{Context, Result};
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;
use std::time::Instant;

use crate::test_model::TestStatus;

/// Event from the test runner
#[derive(Debug, Clone)]
pub enum TestEvent {
    /// Test execution started
    Started { test_name: String },
    /// Test output line
    Output { test_name: String, line: String },
    /// Test completed
    Completed {
        test_name: String,
        status: TestStatus,
        duration_ms: u64,
    },
    /// All tests completed
    AllCompleted {
        passed: usize,
        failed: usize,
        ignored: usize,
    },
    /// Error occurred
    Error { message: String },
}

/// Test runner for executing Rust tests
pub struct TestRunner {
    project_dir: std::path::PathBuf,
    event_tx: Option<Sender<TestEvent>>,
}

impl TestRunner {
    pub fn new(project_dir: &Path) -> Self {
        Self {
            project_dir: project_dir.to_path_buf(),
            event_tx: None,
        }
    }

    /// Set the event sender for real-time updates
    pub fn with_event_sender(mut self, tx: Sender<TestEvent>) -> Self {
        self.event_tx = Some(tx);
        self
    }

    /// Run all tests
    pub fn run_all(&self) -> Result<TestRunResult> {
        self.run_tests_internal(None, false)
    }

    /// Run a single test by name
    pub fn run_test(&self, test_name: &str) -> Result<TestRunResult> {
        self.run_tests_internal(Some(test_name), false)
    }

    /// Run tests matching a filter
    pub fn run_filtered(&self, filter: &str) -> Result<TestRunResult> {
        self.run_tests_internal(Some(filter), false)
    }

    /// Run only failed tests (requires test names)
    pub fn run_tests(&self, test_names: &[String]) -> Result<TestRunResult> {
        let mut total_result = TestRunResult::default();

        for name in test_names {
            let result = self.run_test(name)?;
            total_result.passed += result.passed;
            total_result.failed += result.failed;
            total_result.ignored += result.ignored;
            total_result.test_results.extend(result.test_results);
        }

        Ok(total_result)
    }

    /// Run specific tests by name (alias for run_tests)
    pub fn run_specific(&self, test_names: &[String]) -> Result<TestRunResult> {
        self.run_tests(test_names)
    }

    fn run_tests_internal(&self, filter: Option<&str>, _include_ignored: bool) -> Result<TestRunResult> {
        let start = Instant::now();

        // Build cargo test command
        let mut cmd = Command::new("cargo");
        cmd.arg("test");

        if let Some(f) = filter {
            cmd.arg(f);
        }

        // Use test-threads=1 for deterministic output parsing
        cmd.args(["--", "--test-threads=1"]);

        cmd.current_dir(&self.project_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = cmd.spawn().context("Failed to spawn cargo test")?;

        // Parse output in real-time
        let result = self.parse_test_output(&mut child)?;

        // Wait for process to complete
        let status = child.wait()?;

        let mut result = result;
        result.success = status.success() || result.failed == 0;
        result.duration_ms = start.elapsed().as_millis() as u64;

        // Send completion event
        if let Some(ref tx) = self.event_tx {
            let _ = tx.send(TestEvent::AllCompleted {
                passed: result.passed,
                failed: result.failed,
                ignored: result.ignored,
            });
        }

        Ok(result)
    }

    fn parse_test_output(&self, child: &mut Child) -> Result<TestRunResult> {
        let stdout = child.stdout.take().context("Failed to capture stdout")?;
        let stderr = child.stderr.take().context("Failed to capture stderr")?;

        let mut result = TestRunResult::default();

        // Read stdout for test output
        let reader = BufReader::new(stdout);
        let mut current_test: Option<String> = None;
        let mut current_output: Vec<String> = Vec::new();
        let mut test_start: Option<Instant> = None;

        for line in reader.lines() {
            let line = line?;

            // Parse test start: "test module::test_name ..."
            if line.starts_with("test ") && !line.starts_with("test result:") {
                // Check for test status on same line
                if let Some(test_info) = parse_test_line(&line) {
                    // Emit start event
                    if let Some(ref tx) = self.event_tx {
                        let _ = tx.send(TestEvent::Started {
                            test_name: test_info.name.clone(),
                        });
                    }

                    // If status is immediate (not "...")
                    if test_info.status != TestStatus::Running {
                        let test_result = SingleTestResult {
                            name: test_info.name.clone(),
                            status: test_info.status,
                            duration_ms: Some(0),
                            output: Vec::new(),
                        };

                        match test_info.status {
                            TestStatus::Passed => result.passed += 1,
                            TestStatus::Failed => result.failed += 1,
                            TestStatus::Ignored => result.ignored += 1,
                            _ => {}
                        }

                        // Emit completion event
                        if let Some(ref tx) = self.event_tx {
                            let _ = tx.send(TestEvent::Completed {
                                test_name: test_info.name,
                                status: test_result.status,
                                duration_ms: 0,
                            });
                        }

                        result.test_results.push(test_result);
                    } else {
                        // Test is running, track it
                        current_test = Some(test_info.name);
                        current_output.clear();
                        test_start = Some(Instant::now());
                    }
                }
            }
            // Parse test result line: "test module::test_name ... ok/FAILED"
            else if let Some(ref test_name) = current_test.clone() {
                if line.contains(" ok") || line.contains(" FAILED") || line.contains(" ignored") {
                    let status = if line.contains(" ok") {
                        TestStatus::Passed
                    } else if line.contains(" FAILED") {
                        TestStatus::Failed
                    } else {
                        TestStatus::Ignored
                    };

                    let duration_ms = test_start
                        .map(|s| s.elapsed().as_millis() as u64);

                    let test_result = SingleTestResult {
                        name: test_name.clone(),
                        status,
                        duration_ms,
                        output: current_output.clone(),
                    };

                    match status {
                        TestStatus::Passed => result.passed += 1,
                        TestStatus::Failed => result.failed += 1,
                        TestStatus::Ignored => result.ignored += 1,
                        _ => {}
                    }

                    // Emit completion event
                    if let Some(ref tx) = self.event_tx {
                        let _ = tx.send(TestEvent::Completed {
                            test_name: test_name.clone(),
                            status,
                            duration_ms: duration_ms.unwrap_or(0),
                        });
                    }

                    result.test_results.push(test_result);
                    current_test = None;
                    current_output.clear();
                } else {
                    // Capture output
                    current_output.push(line.clone());

                    // Emit output event
                    if let Some(ref tx) = self.event_tx {
                        let _ = tx.send(TestEvent::Output {
                            test_name: test_name.clone(),
                            line,
                        });
                    }
                }
            }
            // Parse summary line: "test result: ok. X passed; Y failed; Z ignored"
            else if line.starts_with("test result:") {
                // We already tracked individual results
            }
        }

        // Also read stderr for compilation errors
        let stderr_reader = BufReader::new(stderr);
        for line in stderr_reader.lines() {
            if let Ok(line) = line {
                // Emit as error output
                if let Some(ref tx) = self.event_tx {
                    let _ = tx.send(TestEvent::Output {
                        test_name: "compile".to_string(),
                        line,
                    });
                }
            }
        }

        Ok(result)
    }
}

/// Information parsed from a test line
struct TestLineInfo {
    name: String,
    status: TestStatus,
}

/// Parse a line like "test module::test_name ... ok"
fn parse_test_line(line: &str) -> Option<TestLineInfo> {
    if !line.starts_with("test ") {
        return None;
    }

    let rest = &line[5..]; // Skip "test "

    // Find the test name (ends at " ... " or " ..." at end of line)
    let (name, status_part) = if let Some(idx) = rest.find(" ... ") {
        (&rest[..idx], &rest[idx + 5..])
    } else if rest.ends_with(" ...") {
        // Running test - ends with " ..."
        (&rest[..rest.len() - 4], "")
    } else if let Some(idx) = rest.find(" - ") {
        // Doc test format: "test module::func - ... "
        (&rest[..idx], &rest[idx + 3..])
    } else {
        (rest.trim(), "")
    };

    let status = if status_part.is_empty() || status_part.trim() == "..." {
        TestStatus::Running
    } else if status_part.contains("ok") {
        TestStatus::Passed
    } else if status_part.contains("FAILED") {
        TestStatus::Failed
    } else if status_part.contains("ignored") {
        TestStatus::Ignored
    } else {
        TestStatus::Running
    };

    Some(TestLineInfo {
        name: name.to_string(),
        status,
    })
}

/// Result of a single test
#[derive(Debug, Clone)]
pub struct SingleTestResult {
    pub name: String,
    pub status: TestStatus,
    pub duration_ms: Option<u64>,
    pub output: Vec<String>,
}

/// Result of running tests
#[derive(Debug, Clone, Default)]
pub struct TestRunResult {
    pub success: bool,
    pub passed: usize,
    pub failed: usize,
    pub ignored: usize,
    pub duration_ms: u64,
    pub test_results: Vec<SingleTestResult>,
}

impl TestRunResult {
    pub fn total(&self) -> usize {
        self.passed + self.failed + self.ignored
    }
}

/// Create a channel for receiving test events
pub fn create_event_channel() -> (Sender<TestEvent>, Receiver<TestEvent>) {
    channel()
}

/// Async-friendly test runner that spawns tests in a separate thread
pub fn run_tests_async(
    project_dir: &Path,
    filter: Option<String>,
    event_tx: Sender<TestEvent>,
) -> thread::JoinHandle<Result<TestRunResult>> {
    let project_dir = project_dir.to_path_buf();

    thread::spawn(move || {
        let runner = TestRunner::new(&project_dir).with_event_sender(event_tx);

        if let Some(f) = filter {
            runner.run_filtered(&f)
        } else {
            runner.run_all()
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_test_line() {
        let info = parse_test_line("test module::tests::test_one ... ok").unwrap();
        assert_eq!(info.name, "module::tests::test_one");
        assert_eq!(info.status, TestStatus::Passed);

        let info = parse_test_line("test my_test ... FAILED").unwrap();
        assert_eq!(info.name, "my_test");
        assert_eq!(info.status, TestStatus::Failed);

        let info = parse_test_line("test ignored_test ... ignored").unwrap();
        assert_eq!(info.name, "ignored_test");
        assert_eq!(info.status, TestStatus::Ignored);
    }

    #[test]
    fn test_parse_running_test() {
        let info = parse_test_line("test long_test ...").unwrap();
        assert_eq!(info.name, "long_test");
        assert_eq!(info.status, TestStatus::Running);
    }
}
