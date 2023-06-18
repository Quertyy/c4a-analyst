extern crate reqwest;
extern crate serde_json;
extern crate scraper;

use scraper::{Html, Selector};
use std::error::Error;
use std::collections::HashMap;
use regex::Regex;
use chrono::{DateTime, Utc, NaiveDateTime};
use chrono::format::ParseError;
use eyre::Result;
use std::fs::File;
use std::io::BufRead;

use std::process::Command;

use c4a_rust_analyst::contests::*;

fn parse_date(date_str: &str) -> Result<DateTime<Utc>, ParseError> {
    let naive_datetime = NaiveDateTime::parse_from_str(date_str, "%B %d, %Y %H:%M %Z")?;
    Ok(DateTime::from_utc(naive_datetime, Utc))
}

fn extract_end_date(contest_info: &str) -> Option<DateTime<Utc>> {
    let pattern = r"\b(January|February|March|April|May|June|July|August|September|October|November|December)\s+\d{1,2},\s+\d{4}\s+\d{2}:\d{2}\s+UTC\b";
    let re = Regex::new(pattern).unwrap();
    let dates: Vec<&str> = re.find_iter(contest_info).map(|mat| mat.as_str()).collect();

    if let Some(date_str) = dates.last() {
        parse_date(date_str).ok()
    } else {
        None
    }
}

async fn get_contest_scope_contracts(repo_name: &str) -> Vec<String> {
    let client = reqwest::Client::new();
    let git_token = std::env::var("GITHUB_TOKEN").expect("GITHUB_TOKEN must be set");

    let mut headers = reqwest::header::HeaderMap::new();
    
    headers.insert("Authorization", format!("Bearer {}", git_token).parse().unwrap());
    headers.insert("Accept", "application/vnd.github.v3.html".parse().unwrap());
    headers.insert("X-GitHub-Api-Version", "2022-11-28".parse().unwrap());
    headers.insert("User-Agent", "Code-423n4".parse().unwrap());
    let params = [("direction", "desc")];
    let url = format!("https://api.github.com/repos/code-423n4/{}/readme", repo_name);
    //let url = format!("https://api.github.com/repos/code-423n4/2023-05-ajna/readme");
    let contest_info = client.get(url)
        .headers(headers)
        .query(&params)
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    
    let document = Html::parse_fragment(contest_info.as_str());
    let selector = Selector::parse("tr").unwrap();
    let mut contracts_path = Vec::new();
    for element in document.select(&selector) {
        if let Some(link_element) = element.select(&Selector::parse("td a").unwrap()).next() {
            let link_text = link_element.text().collect::<String>();
            if link_text.contains(".sol") {
                if link_text.contains("lib") || link_text.contains("interfaces") || link_text.contains("libraries") {
                    continue;
                }
                //let contract_name = link_text.split('/').last().unwrap(); // Get the last part of the path
                //println!("Found contract: {}", link_text);
                contracts_path.push(link_text);
            }
        }
    }
    contracts_path
}

fn get_pragma_contract_version(contract_path: &str) -> Option<String> {
    let file = File::open(contract_path).expect("failed to open contract file");
    let reader = std::io::BufReader::new(file);
    let re = Regex::new(r"^pragma solidity ([^;]+);").unwrap();
    for line in reader.lines() {
        let line = line.unwrap();
        if let Some(captures) = re.captures(&line) {
            return Some(captures[1].to_string());
        }
    }

    None
}

fn get_contract_bytecode(contract_path: &str, repo_path: &str) -> (String, String) {
    let current_dir = std::env::current_dir().expect("Failed to get current directory");
    let new_dir = current_dir.join(repo_path);
    let parts: Vec<&str> = contract_path.split('/').collect();
    let file_name_with_extension = parts.last().unwrap();
    let contract_name: Vec<&str> = file_name_with_extension.split('.').collect();
    let contract_name = contract_name[0];
    let output = Command::new("forge")
        .current_dir(&new_dir)
        .args(&["inspect"])
        .args(&[contract_name])
        .args(&["bytecode"])
        .output()
        .expect("failed to execute process");
    let bytecode = String::from_utf8_lossy(&output.stdout);
    (contract_name.to_string(), bytecode.replace("\n", "").to_string())
}

fn compile_contracts(path: &str) {
    let current_dir = std::env::current_dir().expect("Failed to get current directory");
    let new_dir = current_dir.join(path);

    let foundry_toml_path = new_dir.join("foundry.toml");
    if foundry_toml_path.exists() {
        println!("Compiling contracts...");
        Command::new("forge")
            .current_dir(&new_dir)
            .args(&["install"])
            .output()
            .expect("failed to installing dependencies");
        Command::new("forge")
            .current_dir(&new_dir)
            .args(&["build"])
            .output()
            .expect("failed to build contracts");
        println!("Contracts compiled successfully!");
    } else {
        println!("No foundry.toml file found in the repository");
    }
}

fn clope_repo(repo_name: &str) {
    println!("Cloning {} repository...", repo_name);
    let url = format!("https://github.com/code-423n4/{}.git", repo_name);
    std::process::Command::new("git")
        .args(&["clone", &url])
        .output()
        .expect("failed to execute git clone");
    println!("Repository cloned successfully!");
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    dotenv::dotenv().ok();
    let client = reqwest::Client::new();
    let git_token = std::env::var("GITHUB_TOKEN").expect("GITHUB_TOKEN must be set");

    let mut headers = reqwest::header::HeaderMap::new();
    
    headers.insert("Authorization", format!("Bearer {}", git_token).parse().unwrap());
    headers.insert("Accept", "application/vnd.github+json".parse().unwrap());
    headers.insert("X-GitHub-Api-Version", "2022-11-28".parse().unwrap());
    headers.insert("User-Agent", "Code-423n4".parse().unwrap());
    let params = [("direction", "desc")];
    let resp = client.get("https://api.github.com/orgs/code-423n4/repos")
        .headers(headers)
        .query(&params)
        .send()
        .await?
        .text()
        .await?;
    
    let repos: Vec<HashMap<String, serde_json::Value>> = serde_json::from_str(&resp)?;
    let mut contests = Contests::new();
    for repo in repos {
        let name = repo.get("name").unwrap().as_str().unwrap();
        
        if name.contains("2023") {
            let client = reqwest::Client::new();
            let url = format!("https://raw.githubusercontent.com/code-423n4/{}/main/README.md", name);
            let contest_info = client.get(&url)
                .send()
                .await?
                .text()
                .await?;
            let end_date = extract_end_date(&contest_info);
            let current_timestamp = chrono::Utc::now().timestamp();
            
            match end_date {
                Some(date) => {
                    if date.timestamp() < current_timestamp {
                        continue;
                    } else {
                        let mut contest = Contest::new(name);

                        println!("{}: {}", name, date);
                        let contracts_path = get_contest_scope_contracts(name).await;
                        clope_repo(name);
                        let path = std::path::Path::new(name);
                        compile_contracts(path.to_str().unwrap());
                        println!("Found {} contracts", contracts_path.len());
                        println!("Getting pragma versions and bytecodes from contracts...");
                        for path in contracts_path {
                            let contract_path = format!("{}/{}", name, path);
                            let pragma_version = get_pragma_contract_version(&contract_path);
                            let (contract_name, bytecode) = get_contract_bytecode(&path, name);

                            match pragma_version {
                                Some(version) => {
                                    let contract = Contract::new(contract_name, bytecode, version);
                                    contest.add_contract(contract);
                                },
                                None => {
                                    println!("{}: No pragma version found", contract_path);
                                }
                            }
                        }
                        println!("Jobs done!");
                        contests.add_contest(contest);
                    }
                },
                None => {
                    continue;
                }
            }
        }
        
    }
    println!("Exporting contests info...");
    export_contest_info(contests);
    println!("Contests info exported successfully!");
    Ok(())
}

