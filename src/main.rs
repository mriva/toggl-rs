use std::collections::HashMap;
use std::default::Default;
use reqwest::Method;
use chrono::DateTime;
use serde::{Serialize, Deserialize};

mod client;

#[derive(Serialize, Deserialize)]
pub struct Config {
    workspace_id: String,
    clients: HashMap<String, Client>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            workspace_id: String::new(),
            clients: HashMap::new(),
        }
    }
}

#[derive(Deserialize, Debug)]
struct TimeEntry {
    start: String,
    end: String,
}

#[derive(Debug, serde::Deserialize)]
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

struct BillReportDay {
    date: String,
    actual_minutes: i64,
    billed_minutes: i64,
    billed_amount: f64,
}

struct BillReport {
    days: Vec<BillReportDay>,
}

fn main() -> Result<(), anyhow::Error> {
    let config: Config = confy::load_path("./config.toml")?;
    let client_name = std::env::args().nth(1).ok_or(anyhow::anyhow!("No client name provided"))?;

    let client = &config.clients[&client_name];
    let url = "https://api.track.toggl.com/reports/api/v2/details";

    let mut req_query: HashMap<&str, &str> = HashMap::new();
    req_query.insert("client_ids", &client.id);
    req_query.insert("since", &client.last_billed_date);

    let bill_report = client::make_request(Method::GET, url, req_query, &config)
        .and_then(|res| serde_json::from_str::<ReportDetails>(&res).map_err(|e| anyhow::anyhow!(e)))
        .map(build_summary)
        .map(|summary| build_bill_report(summary, &client))?;

    let mut total = 0.0;

    for day in bill_report.days {
        total += day.billed_amount;
        println!("{} - {} - {} - {}", day.date, day.actual_minutes, day.billed_minutes, day.billed_amount);
    }

    println!("Total: {}", total);

    Ok(())
}

fn build_summary(report_details: ReportDetails) -> Summary {
    let mut summary: Summary = Summary::new();

    for entry in report_details.data {
        let start = DateTime::parse_from_rfc3339(&entry.start).unwrap();
        let end = DateTime::parse_from_rfc3339(&entry.end).unwrap();
        let diff = end - start;
        let day = start.format("%Y-%m-%d").to_string();

        summary.entry(day).and_modify(|x| *x += diff.num_minutes()).or_insert(diff.num_minutes());
    }

    summary
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

        let amount = billable_minutes as f64 * &client.hourly_rate / 60.0;
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
