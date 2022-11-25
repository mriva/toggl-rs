use std::collections::HashMap;
use std::default::Default;
use anyhow::{Result, Context};
use reqwest::Method;
use chrono::DateTime;
use serde::{Serialize, Deserialize};

mod client;

#[derive(Serialize, Deserialize, Default)]
pub struct Config {
    workspace_id: String,
    clients: HashMap<String, Client>,
}

#[derive(Deserialize, Debug, Clone)]
struct TimeEntry {
    start: String,
    end: String,
}

#[derive(Debug, serde::Deserialize, Clone)]
struct ReportDetails {
    data: Vec<TimeEntry>,
}

#[derive(Serialize, Deserialize, Debug)]
struct Client {
    id: String,
    hourly_rate: f64,
    last_billed_date: String,
}

type Summary = HashMap<String, i64>;

#[derive(Debug, PartialEq)]
struct BillReportDay {
    date: String,
    actual_minutes: i64,
    billed_minutes: i64,
    billed_amount: f64,
}

#[derive(Debug, PartialEq)]
struct BillReport {
    days: Vec<BillReportDay>,
}

fn main() -> Result<()> {
    let config: Config = confy::load_path("./config.toml")?;
    let client_name = std::env::args().nth(1).ok_or_else(|| anyhow::anyhow!("No client name provided"))?;

    let client = &config.clients[&client_name];
    let url = "https://api.track.toggl.com/reports/api/v2/details";

    let mut req_query: HashMap<&str, &str> = HashMap::new();
    req_query.insert("client_ids", &client.id);
    req_query.insert("since", &client.last_billed_date);

    let bill_report = client::make_request(Method::GET, url, req_query, &config)
        .and_then(|res| serde_json::from_str::<ReportDetails>(&res).map_err(|e| anyhow::anyhow!(e)))
        .and_then(|r| build_summary(&r))
        .map(|summary| build_bill_report(summary, client))?;

    let mut total = 0.0;

    for day in bill_report.days {
        total += day.billed_amount;
        println!("{} - {} - {} - {}", day.date, day.actual_minutes, day.billed_minutes, day.billed_amount);
    }

    println!("Total: {}", total);

    Ok(())
}

fn build_summary(report_details: &ReportDetails) -> Result<Summary> {
    let mut summary: Summary = Summary::new();

    for entry in &report_details.data {
        let start = DateTime::parse_from_rfc3339(&entry.start).with_context(|| format!("Failed to parse start date: {}", entry.start))?;
        let end = DateTime::parse_from_rfc3339(&entry.end).with_context(|| format!("Failed to parse end date: {}", entry.end))?;
        let diff = end - start;
        let day = start.format("%Y-%m-%d").to_string();

        summary.entry(day).and_modify(|x| *x += diff.num_minutes()).or_insert_with(|| diff.num_minutes());
    }

    Ok(summary)
}

fn build_bill_report(summary: Summary, client: &Client) -> BillReport {
    let mut bill_report = BillReport {
        days: Vec::new(),
    };
    
    for (day, minutes) in summary {
        let billable_minutes: i64 = match minutes {
            0 ..= 10 => 0,
            11 ..= 60 => 60,
            _ => minutes
        };

        let amount = billable_minutes as f64 * client.hourly_rate / 60.0;
        bill_report.days.push(BillReportDay {
            date: day,
            actual_minutes: minutes,
            billed_minutes: billable_minutes,
            billed_amount: amount,
        });
    }

    bill_report.days.sort_by_key(|x| x.date.clone());

    bill_report
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_build_summary() {
        let report_details = ReportDetails {
            data: vec![
                TimeEntry { start: "2022-01-01T00:00:00+00:00".to_string(), end: "2022-01-01T00:10:00+00:00".to_string() },
                TimeEntry { start: "2022-01-01T10:00:00+00:00".to_string(), end: "2022-01-01T11:10:00+00:00".to_string() },
                TimeEntry { start: "2022-02-01T15:00:00+00:00".to_string(), end: "2022-02-01T15:52:00+00:00".to_string() },
            ]
        };
        let mut summary = Summary::new();
        summary.insert("2022-01-01".to_string(), 80);
        summary.insert("2022-02-01".to_string(), 52);

        assert_eq!(summary, build_summary(&report_details).unwrap());
    }

    #[test]
    fn test_build_summary_invalid_date() {
        let report_details = ReportDetails {
            data: vec![
                TimeEntry { start: "this string is not a date".to_string(), end: "2022-01-01T00:10:00+00:00".to_string() },
            ]
        };

        assert!(build_summary(&report_details).is_err());
        assert_eq!("Failed to parse start date: this string is not a date".to_string(), build_summary(&report_details).unwrap_err().to_string());

        let report_details = ReportDetails {
            data: vec![
                TimeEntry { start: "2022-01-01T00:10:00+00:00".to_string(), end: "this string is not a date".to_string() },
            ]
        };

        assert!(build_summary(&report_details).is_err());
        assert_eq!("Failed to parse end date: this string is not a date".to_string(), build_summary(&report_details).unwrap_err().to_string());
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
                },
                BillReportDay {
                    date: "2022-01-02".to_string(),
                    actual_minutes: 25,
                    billed_minutes: 60,
                    billed_amount: 30.0,
                },
                BillReportDay {
                    date: "2022-01-03".to_string(),
                    actual_minutes: 80,
                    billed_minutes: 80,
                    billed_amount: 80.0 / 60.0 * 30.0,
                },
            ]
        };

        let client = Client {
            id: "123".to_string(),
            hourly_rate: 30.0,
            last_billed_date: "2022-01-01".to_string(),
        };

        assert_eq!(expected_bill_report, build_bill_report(summary, &client));
    }
}
