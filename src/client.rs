use std::collections::HashMap;

use chrono::{Months, NaiveDate, Datelike};
use reqwest::{ blocking::Client, Method };
use anyhow::Result;
use crate::{DetailsResponse, TimeEntry};

use super::{ Config, ReportDetails };

struct ReportYear {
    current: usize,
    until: usize,
}

impl ReportYear {
    fn new(current: usize, until: Option<usize>) -> Self {
        let until = until.unwrap_or(chrono::Local::now().year() as usize);
        Self { current, until }
    }
}

impl Iterator for ReportYear {
    type Item = (String, String);

    fn next(&mut self) -> Option<Self::Item> {
        if self.current > self.until {
            None
        } else {
            let current_from = format!("{}-01-01", self.current);
            let current_to = format!("{}-12-31", self.current);
            self.current += 1;
            Some((current_from, current_to))
        }
    }
}

pub fn get_billable_report(config: &Config, client_name: &str) -> Result<ReportDetails> {
    let mut full_report = ReportDetails {
        data: Vec::new(),
    };

    for (since, until) in ReportYear::new(2022, None) {
        let mut year_report = get_year_data(config, client_name, since, until)?;
        full_report.data.append(&mut year_report);
    }

    Ok(full_report)
}

fn get_year_data(config: &Config, client_name: &str, since: String, until: String) -> Result<Vec<crate::TimeEntry>> {
    let client = &config.clients[client_name];
    let url = "https://api.track.toggl.com/reports/api/v2/details";

    let mut req_query: HashMap<&str, &str> = HashMap::new();
    req_query.insert("client_ids", &client.id);
    req_query.insert("since", &since);
    req_query.insert("until", &until);

    let mut entries: Vec<TimeEntry> = Vec::new();

    let mut response = make_request(Method::GET, url, req_query, config)
        .and_then(|r| serde_json::from_str::<DetailsResponse>(&r).map_err(|e| anyhow::anyhow!(e)))?;

    entries.append(&mut response.data);

    if response.total_count > 50 {
        let mut page = 2;
        let mut total_pages = response.total_count / 50;
        if response.total_count % 50 > 0 {
            total_pages += 1;
        }

        while page <= total_pages {
            let query_page = page.to_string();

            let mut req_query: HashMap<&str, &str> = HashMap::new();
            req_query.insert("client_ids", &client.id);
            req_query.insert("since", &since);
            req_query.insert("until", &until);
            req_query.insert("page", &query_page);

            let mut response = make_request(Method::GET, url, req_query, config)
                .and_then(|r| serde_json::from_str::<DetailsResponse>(&r).map_err(|e| anyhow::anyhow!(e)))?;

            entries.append(&mut response.data);
            page += 1;
        }
    }

    println!("Got {} entries for {} - {}", response.total_count, since, until);
    Ok(entries)
}

fn make_request(method: Method, url: &str, query_params: HashMap<&str, &str>, config: &Config) -> Result<String> {
    let client = Client::new();

    let token = base64::encode(format!("{}:api_token", std::env::var("TOGGLE_API_TOKEN")?));

    let mut base_params = HashMap::new();
    base_params.insert("user_agent", "toggl-rs");
    base_params.insert("workspace_id", &config.workspace_id);
    
    client.request(method, url)
        .header("Authorization", format!("Basic {}", token))
        .header("Content-Type", "application/json")
        .query(&base_params)
        .query(&query_params)
        .send()
        .and_then(|r| r.text())
        .map_err(|e| anyhow::anyhow!(e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_year_iterator() {
        let mut report_year = ReportYear::new(2018, Some(2022));

        assert_eq!(report_year.next(), Some(("2018-01-01".to_string(), "2018-12-31".to_string())));
        assert_eq!(report_year.next(), Some(("2019-01-01".to_string(), "2019-12-31".to_string())));
        assert_eq!(report_year.next(), Some(("2020-01-01".to_string(), "2020-12-31".to_string())));
        assert_eq!(report_year.next(), Some(("2021-01-01".to_string(), "2021-12-31".to_string())));
        assert_eq!(report_year.next(), Some(("2022-01-01".to_string(), "2022-12-31".to_string())));
        assert_eq!(report_year.next(), None);
    }

    #[test]
    fn test_year_iterator_without_end_year() {
        let mut report_year = ReportYear::new(2018, None);

        let current_year = chrono::Local::now().year() as usize;
        for year in 2018..=current_year {
            assert_eq!(report_year.next(), Some((format!("{}-01-01", year), format!("{}-12-31", year))));
        }

        assert_eq!(None, report_year.next());
    }
}
