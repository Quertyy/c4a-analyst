use serde::{Serialize, Deserialize};
use serde_json;
use std::fs::OpenOptions;
use std::io::prelude::*;

#[derive(Serialize, Deserialize, Debug)]
pub struct Contract {
    pub name: String,
    pub bytecode: String,
    pub pragma_version: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Contest {
    pub name: String,
    pub contracts: Vec<Contract>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Contests {
    pub contests: Vec<Contest>,
}

impl Contract {
    pub fn new(name: String, bytecode: String, pragma_version: String) -> Self {
        Contract {
            name,
            bytecode,
            pragma_version,
        }
    }
}

impl Contest {
    pub fn new(name: &str) -> Self {
        Contest {
            name: String::from(name),
            contracts: Vec::new(),
        }
    }

    pub fn add_contract(&mut self, contract: Contract) {
        self.contracts.push(contract);
    }
}

impl Contests {
    pub fn new() -> Self {
        Contests {
            contests: Vec::new(),
        }
    }

    pub fn add_contest(&mut self, contest: Contest) {
        self.contests.push(contest);
    }
}

pub fn export_contest_info(contests: Contests) {
    let json = serde_json::to_string_pretty(&contests).unwrap();
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .open("contest_info.json")
        .unwrap();
    file.write_all(json.as_bytes()).unwrap();
}