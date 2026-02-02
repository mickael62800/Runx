use anyhow::Result;
use quick_xml::events::Event;
use quick_xml::Reader;
use std::fs;
use std::path::Path;

use crate::db::TestCase;

#[derive(Debug, Default)]
pub struct JUnitTestSuite {
    pub name: String,
    pub tests: i32,
    pub failures: i32,
    pub errors: i32,
    pub skipped: i32,
    pub time: f64,
    pub test_cases: Vec<JUnitTestCase>,
}

#[derive(Debug, Default, Clone)]
pub struct JUnitTestCase {
    pub name: String,
    pub classname: Option<String>,
    pub time: Option<f64>,
    pub status: TestStatus,
    pub failure_message: Option<String>,
    pub failure_type: Option<String>,
    pub error_message: Option<String>,
    pub error_type: Option<String>,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub enum TestStatus {
    #[default]
    Passed,
    Failed,
    Error,
    Skipped,
}

impl ToString for TestStatus {
    fn to_string(&self) -> String {
        match self {
            TestStatus::Passed => "passed".to_string(),
            TestStatus::Failed => "failed".to_string(),
            TestStatus::Error => "error".to_string(),
            TestStatus::Skipped => "skipped".to_string(),
        }
    }
}

pub fn parse_junit_xml(path: &Path) -> Result<Vec<JUnitTestSuite>> {
    let content = fs::read_to_string(path)?;
    parse_junit_string(&content)
}

pub fn parse_junit_string(xml: &str) -> Result<Vec<JUnitTestSuite>> {
    let mut reader = Reader::from_str(xml);
    reader.trim_text(true);

    let mut suites = Vec::new();
    let mut current_suite: Option<JUnitTestSuite> = None;
    let mut current_case: Option<JUnitTestCase> = None;
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                match e.name().as_ref() {
                    b"testsuite" => {
                        let mut suite = JUnitTestSuite::default();
                        for attr in e.attributes().filter_map(|a| a.ok()) {
                            match attr.key.as_ref() {
                                b"name" => suite.name = String::from_utf8_lossy(&attr.value).to_string(),
                                b"tests" => suite.tests = String::from_utf8_lossy(&attr.value).parse().unwrap_or(0),
                                b"failures" => suite.failures = String::from_utf8_lossy(&attr.value).parse().unwrap_or(0),
                                b"errors" => suite.errors = String::from_utf8_lossy(&attr.value).parse().unwrap_or(0),
                                b"skipped" => suite.skipped = String::from_utf8_lossy(&attr.value).parse().unwrap_or(0),
                                b"time" => suite.time = String::from_utf8_lossy(&attr.value).parse().unwrap_or(0.0),
                                _ => {}
                            }
                        }
                        current_suite = Some(suite);
                    }
                    b"testcase" => {
                        let mut case = JUnitTestCase::default();
                        for attr in e.attributes().filter_map(|a| a.ok()) {
                            match attr.key.as_ref() {
                                b"name" => case.name = String::from_utf8_lossy(&attr.value).to_string(),
                                b"classname" => case.classname = Some(String::from_utf8_lossy(&attr.value).to_string()),
                                b"time" => case.time = String::from_utf8_lossy(&attr.value).parse().ok(),
                                _ => {}
                            }
                        }
                        current_case = Some(case);
                    }
                    b"failure" => {
                        if let Some(ref mut case) = current_case {
                            case.status = TestStatus::Failed;
                            for attr in e.attributes().filter_map(|a| a.ok()) {
                                match attr.key.as_ref() {
                                    b"message" => case.failure_message = Some(String::from_utf8_lossy(&attr.value).to_string()),
                                    b"type" => case.failure_type = Some(String::from_utf8_lossy(&attr.value).to_string()),
                                    _ => {}
                                }
                            }
                        }
                    }
                    b"error" => {
                        if let Some(ref mut case) = current_case {
                            case.status = TestStatus::Error;
                            for attr in e.attributes().filter_map(|a| a.ok()) {
                                match attr.key.as_ref() {
                                    b"message" => case.error_message = Some(String::from_utf8_lossy(&attr.value).to_string()),
                                    b"type" => case.error_type = Some(String::from_utf8_lossy(&attr.value).to_string()),
                                    _ => {}
                                }
                            }
                        }
                    }
                    b"skipped" => {
                        if let Some(ref mut case) = current_case {
                            case.status = TestStatus::Skipped;
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(ref e)) => {
                match e.name().as_ref() {
                    b"testcase" => {
                        // Self-closing testcase = passed test
                        let mut case = JUnitTestCase::default();
                        for attr in e.attributes().filter_map(|a| a.ok()) {
                            match attr.key.as_ref() {
                                b"name" => case.name = String::from_utf8_lossy(&attr.value).to_string(),
                                b"classname" => case.classname = Some(String::from_utf8_lossy(&attr.value).to_string()),
                                b"time" => case.time = String::from_utf8_lossy(&attr.value).parse().ok(),
                                _ => {}
                            }
                        }
                        if let Some(ref mut suite) = current_suite {
                            suite.test_cases.push(case);
                        }
                    }
                    b"skipped" => {
                        if let Some(ref mut case) = current_case {
                            case.status = TestStatus::Skipped;
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) => {
                match e.name().as_ref() {
                    b"testsuite" => {
                        if let Some(suite) = current_suite.take() {
                            suites.push(suite);
                        }
                    }
                    b"testcase" => {
                        if let (Some(ref mut suite), Some(case)) = (&mut current_suite, current_case.take()) {
                            suite.test_cases.push(case);
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(e)) => {
                let text = e.unescape().unwrap_or_default().to_string();
                if !text.trim().is_empty() {
                    if let Some(ref mut case) = current_case {
                        if case.status == TestStatus::Failed && case.failure_message.is_none() {
                            case.failure_message = Some(text);
                        } else if case.status == TestStatus::Error && case.error_message.is_none() {
                            case.error_message = Some(text);
                        }
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(anyhow::anyhow!("Error parsing JUnit XML: {}", e)),
            _ => {}
        }
        buf.clear();
    }

    Ok(suites)
}

pub fn junit_to_test_cases(suites: &[JUnitTestSuite], task_result_id: &str) -> Vec<TestCase> {
    let mut cases = Vec::new();

    for suite in suites {
        for tc in &suite.test_cases {
            cases.push(TestCase {
                id: 0,
                task_result_id: task_result_id.to_string(),
                name: tc.name.clone(),
                classname: tc.classname.clone(),
                status: tc.status.to_string(),
                duration_ms: tc.time.map(|t| (t * 1000.0) as i64),
                error_message: tc.failure_message.clone().or_else(|| tc.error_message.clone()),
                error_type: tc.failure_type.clone().or_else(|| tc.error_type.clone()),
            });
        }
    }

    cases
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_junit() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<testsuite name="MyTests" tests="3" failures="1" errors="0" skipped="1" time="1.234">
    <testcase name="test_pass" classname="MyClass" time="0.100"/>
    <testcase name="test_fail" classname="MyClass" time="0.200">
        <failure message="assertion failed" type="AssertionError">Expected true but got false</failure>
    </testcase>
    <testcase name="test_skip" classname="MyClass" time="0.001">
        <skipped/>
    </testcase>
</testsuite>"#;

        let suites = parse_junit_string(xml).unwrap();
        assert_eq!(suites.len(), 1);
        assert_eq!(suites[0].name, "MyTests");
        assert_eq!(suites[0].test_cases.len(), 3);
    }
}
