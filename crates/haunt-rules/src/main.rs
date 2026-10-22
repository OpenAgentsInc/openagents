//! `haunt`: read and check a Haunted House document.
//!
//! ```text
//! haunt describe [FILE]         the map and every area, for an agent
//! haunt describe --library      the prepared library
//! haunt check [FILE]            the checks; exits 1 with the refusal
//! haunt add-area FILE NEXT_TO WALL KIND WIDTH DEPTH FEELING
//!                               prints the house with a new area, checked
//! haunt remove-area FILE ID     prints the house without an area, checked
//! ```
//!
//! Without a file it reads version 1, the MVP house.

use std::process::ExitCode;

use haunt_rules::doc::{AreaKind, House, Intent, Wall};
use haunt_rules::edit::{NewArea, add_area, remove_area};

fn read(path: Option<&String>) -> Result<House, String> {
    match path {
        None => Ok(haunt_rules::v1()),
        Some(path) => {
            let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
            House::parse(&text)
        }
    }
}

fn checked(house: House) -> Result<String, String> {
    haunt_rules::check(&house, haunt_assets::catalog()).map_err(|r| r.message)?;
    Ok(house.to_json())
}

fn run(args: &[String]) -> Result<String, String> {
    let catalog = haunt_assets::catalog();
    match args.first().map(String::as_str) {
        Some("describe") if args.get(1).map(String::as_str) == Some("--library") => {
            Ok(catalog.describe())
        }
        Some("describe") => Ok(haunt_rules::describe::describe(
            &read(args.get(1))?,
            catalog,
        )),
        Some("check") => {
            let house = read(args.get(1))?;
            haunt_rules::check(&house, catalog).map_err(|r| r.message)?;
            Ok("The house passes every check.\n".into())
        }
        Some("add-area") => {
            let [_, file, next_to, wall, kind, w, d, feeling] = args else {
                return Err(
                    "usage: haunt add-area FILE NEXT_TO WALL KIND WIDTH DEPTH FEELING".into(),
                );
            };
            let house = read(Some(file))?;
            let wall = Wall::parse(wall).ok_or("WALL is north, east, south or west")?;
            let kind = match kind.as_str() {
                "room" => AreaKind::Room,
                "hallway" => AreaKind::Hallway,
                _ => return Err("KIND is room or hallway".into()),
            };
            let size = [
                w.parse().map_err(|_| "WIDTH is metres")?,
                d.parse().map_err(|_| "DEPTH is metres")?,
            ];
            let new = NewArea {
                id: None,
                kind,
                size,
                intent: Intent {
                    feeling: feeling.clone(),
                    beat: String::new(),
                    intensity: [2, 2],
                },
                ceiling: None,
            };
            let (house, _) = add_area(&house, catalog, next_to, wall, &new).map_err(|r| r.message)?;
            checked(house)
        }
        Some("remove-area") => {
            let [_, file, id] = args else {
                return Err("usage: haunt remove-area FILE ID".into());
            };
            checked(remove_area(&read(Some(file))?, id).map_err(|r| r.message)?)
        }
        _ => Err("usage: haunt describe [--library|FILE] | check [FILE] | add-area ... | remove-area FILE ID".into()),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(text) => {
            print!("{text}");
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}
