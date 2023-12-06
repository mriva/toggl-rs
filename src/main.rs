use anyhow::{Context, Result};
use chrono::DateTime;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::default::Default;
use tabled::{settings::Style, Table, Tabled};

mod client;

#[derive(Serialize, Deserialize, Default)]
pub struct Config {
    workspace_id: String,
    start_of_time: String,
    clients: HashMap<String, Client>,
}

#[derive(Deserialize, Debug, Clone)]
struct TimeEntry {
    start: String,
    end: String,
}

#[derive(Debug, serde::Deserialize, Clone)]
pub struct ReportDetails {
    data: Vec<TimeEntry>,
}

#[derive(Debug, serde::Deserialize, Clone)]
pub struct DetailsResponse {
    data: Vec<TimeEntry>,
    total_count: u32,
}

#[derive(Serialize, Deserialize, Debug)]
struct Client {
    id: String,
    hourly_rate: f64,
    last_billed_date: String,
}

type Summary = HashMap<String, i64>;

#[derive(Debug, PartialEq, Tabled)]
struct BillReportDay {
    date: String,
    actual_minutes: i64,
    billed_minutes: i64,
    billed_amount: f64,
    billed: bool,
}

#[derive(Debug, PartialEq)]
struct BillReport {
    days: Vec<BillReportDay>,
}

fn main() -> Result<()> {
    let config: Config = confy::load_path("./config.toml")?;
    let client_name = std::env::args()
        .nth(1)
        .ok_or_else(|| anyhow::anyhow!("No client name provided"))?;
    let client = &config.clients[&client_name];

    let bill_report = client::get_billable_report(&config, &client_name)
        .and_then(|r| build_summary(&r))
        .map(|summary| build_bill_report(summary, client))?;

    let total_minutes = calculate_minutes(&bill_report);
    // division by 60 rounded up
    let total_hours = (total_minutes + 59) / 60;

    let mut table = Table::new(&bill_report.days);

    println!("{}", table.with(Style::sharp()));
    println!("Total minutes: {}", total_minutes);
    println!("Total hours: {}", total_hours);
    println!("Total amount: € {}", total_hours * 30);

    Ok(())
}

fn calculate_minutes(bill_report: &BillReport) -> i64 {
    bill_report.days.iter().fold(0, |acc, day| {
        if day.billed {
            acc
        } else {
            acc + day.billed_minutes
        }
    })
}

fn build_summary(report_details: &ReportDetails) -> Result<Summary> {
    let mut summary: Summary = Summary::new();

    for entry in &report_details.data {
        let start = DateTime::parse_from_rfc3339(&entry.start)
            .with_context(|| format!("Failed to parse start date: {}", entry.start))?;
        let end = DateTime::parse_from_rfc3339(&entry.end)
            .with_context(|| format!("Failed to parse end date: {}", entry.end))?;
        let diff = end - start;
        let day = start.format("%Y-%m-%d").to_string();

        summary
            .entry(day)
            .and_modify(|x| *x += diff.num_minutes())
            .or_insert_with(|| diff.num_minutes());
    }

    Ok(summary)
}

fn build_bill_report(summary: Summary, client: &Client) -> BillReport {
    let mut bill_report = BillReport { days: Vec::new() };

    for (day, minutes) in summary {
        let billable_minutes: i64 = calculate_billable_minutes(minutes);

        let mut billed = false;
        if day <= client.last_billed_date {
            billed = true;
        }

        bill_report.days.push(BillReportDay {
            date: day,
            actual_minutes: minutes,
            billed_minutes: billable_minutes,
            billed_amount: billable_minutes as f64 * client.hourly_rate / 60.0,
            billed,
        });
    }

    bill_report.days.sort_by_key(|x| x.date.clone());

    bill_report
}

fn calculate_billable_minutes(minutes: i64) -> i64 {
    match minutes {
        0..=10 => 0,
        11..=60 => 60,
        61..=70 => minutes,
        71..=120 => 120,
        _ => minutes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_build_summary() {
        let report_details = ReportDetails {
            data: vec![
                TimeEntry {
                    start: "2022-01-01T00:00:00+00:00".to_string(),
                    end: "2022-01-01T00:10:00+00:00".to_string(),
                },
                TimeEntry {
                    start: "2022-01-01T10:00:00+00:00".to_string(),
                    end: "2022-01-01T11:10:00+00:00".to_string(),
                },
                TimeEntry {
                    start: "2022-02-01T15:00:00+00:00".to_string(),
                    end: "2022-02-01T15:52:00+00:00".to_string(),
                },
            ],
        };
        let mut summary = Summary::new();
        summary.insert("2022-01-01".to_string(), 80);
        summary.insert("2022-02-01".to_string(), 52);

        assert_eq!(summary, build_summary(&report_details).unwrap());
    }

    #[test]
    fn test_build_summary_invalid_date() {
        let report_details = ReportDetails {
            data: vec![TimeEntry {
                start: "this string is not a date".to_string(),
                end: "2022-01-01T00:10:00+00:00".to_string(),
            }],
        };

        assert!(build_summary(&report_details).is_err());
        assert_eq!(
            "Failed to parse start date: this string is not a date".to_string(),
            build_summary(&report_details).unwrap_err().to_string()
        );

        let report_details = ReportDetails {
            data: vec![TimeEntry {
                start: "2022-01-01T00:10:00+00:00".to_string(),
                end: "this string is not a date".to_string(),
            }],
        };

        assert!(build_summary(&report_details).is_err());
        assert_eq!(
            "Failed to parse end date: this string is not a date".to_string(),
            build_summary(&report_details).unwrap_err().to_string()
        );
    }

    #[test]
    fn build_bill_report_test() {
        let mut summary = Summary::new();
        summary.insert("2022-01-01".to_string(), 5);
        summary.insert("2022-01-02".to_string(), 25);
        summary.insert("2022-01-03".to_string(), 80);

        let expected_bill_report = BillReport {
            days: vec![
                BillReportDay {
                    date: "2022-01-01".to_string(),
                    actual_minutes: 5,
                    billed_minutes: 0,
                    billed_amount: 0.0,
                    billed: true,
                },
                BillReportDay {
                    date: "2022-01-02".to_string(),
                    actual_minutes: 25,
                    billed_minutes: 60,
                    billed_amount: 30.0,
                    billed: false,
                },
                BillReportDay {
                    date: "2022-01-03".to_string(),
                    actual_minutes: 80,
                    billed_minutes: 80,
                    billed_amount: 80.0 / 60.0 * 30.0,
                    billed: false,
                },
            ],
        };

        let client = Client {
            id: "123".to_string(),
            hourly_rate: 30.0,
            last_billed_date: "2022-01-01".to_string(),
        };

        assert_eq!(expected_bill_report, build_bill_report(summary, &client));
    }
}
