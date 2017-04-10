use std::collections::HashSet;
use futures::future;
use kabuki::Actor;

pub struct Nameserver {
    names: HashSet<String>,
}

impl Actor for Nameserver {
    type Request = String;
    type Response = String;
    type Error = ();
    type Future = future::FutureResult<String, ()>;

    fn call(&mut self, desired_name: Self::Request) -> Self::Future {
        future::ok(self.uniqueify(desired_name))
    }
}

impl Nameserver {
    pub fn uniqueify(&mut self, desired_name: String) -> String {
        let unique_name = self.find_unused_name(desired_name);
        self.names.insert(unique_name.clone());
        unique_name
    }

    fn find_unused_name(&mut self, desired_name: String) -> String {
        if !self.names.contains(&desired_name) {
            return desired_name;
        }

        let mut unique_name = String::new();
        for n in 1.. {
            let potential_name = format!("{}_{}", desired_name, roman_numerals(n));
            if !self.names.contains(&potential_name) {
                unique_name = potential_name;
                break;
            }
        }
        unique_name
    }
}

pub fn roman_numerals(mut value: u64) -> String {
    let mut numerals = "".to_string();
    while value > 0 {
        let (numeral, sub) = match value {
            v if v >= 1000 => ("M", 1000),
            v if v >= 500 => ("D", 500),
            v if v >= 100 => ("C", 100),
            v if v >= 50 => ("L", 50),
            v if v >= 10 => ("X", 10),
            v if v >= 5 => ("V", 5),
            v if v >= 1 => ("I", 1),
            _ => break,
        };
        numerals.push_str(numeral);
        value -= sub;
    }
    numerals
}
