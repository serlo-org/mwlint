//! A REST service for mediawiki source code linting.

#![feature(plugin, custom_derive)]
#![plugin(rocket_codegen)]

extern crate rocket;
extern crate serde_json;
extern crate mwlint;
extern crate mediawiki_parser;
extern crate mwparser_utils;
#[macro_use]
extern crate serde_derive;
extern crate serde;
#[macro_use]
extern crate lazy_static;

use mediawiki_parser::*;
use mwlint::*;
use mwparser_utils::util::CachedTexChecker;
use serde::{Serialize};
use serde_json::Value;
use std::env;
use std::path::PathBuf;

use rocket::http::Status;
use rocket::response::{self, Responder, content};
use rocket::http::Header;
use rocket::request::Request;


lazy_static! {
    static ref SETTINGS: Settings<'static> = {
        let mut settings = Settings::default();
        if let Ok(path) = env::var("TEXVCCHECK_PATH") {
            eprintln!("using {}", &path);
            settings.tex_checker = Some(
                CachedTexChecker::new(&PathBuf::from(path), 100_000)
            );
        } else {
            eprintln!("not checking formulas...");
        }
        settings
    };
}

#[get("/")]
fn index() -> &'static str {
    "no GET endpoints available. Use POST to send mediawiki source code. \
    see https://github.com/vroland/mwlint."
}

#[derive(Debug, Serialize)]
enum ResultKind {
    Lints(Vec<Lint>),
    Error(MWError),
}


#[derive(Debug)]
pub struct Json<T = Value>(pub T);

/// Added CORS headers to JSON contrib implementation.
impl<T: Serialize> Responder<'static> for Json<T> {
    fn respond_to(self, req: &Request) -> response::Result<'static> {
        serde_json::to_string(&self.0).map(|string| {
            {
                let mut rsp = content::Json(string).respond_to(req).unwrap();
                rsp.set_header(Header::new("Access-Control-Allow-Origin", "*"));
                rsp
            }
        }).map_err(|_e| {
            Status::InternalServerError
        })
    }
}

#[get("/examples")]
fn examples() -> Json<Vec<Example>> {
    let rules = get_rules();
    Json(rules.iter().fold(vec![], |mut vec, rule| {
            vec.append(&mut rule.examples().clone());
            vec
        })
    )
}

#[post("/", data = "<source>")]
fn lint(source: String) -> Json<ResultKind> {

    let mut tree = match parse(&source) {
        Ok(elem) => elem,
        Err(mwerror) => return Json(ResultKind::Error(mwerror)),
    };

    tree = match normalize(tree, &SETTINGS) {
        Ok(elem) => elem,
        Err(mwerror) => return Json(
            ResultKind::Error(MWError::TransformationError(mwerror))
        ),
    };

    let mut rules = get_rules();
    let mut lints = vec![];

    for mut rule in &mut rules {
        rule.run(&tree, &SETTINGS, &mut vec![])
            .expect("error while checking rule!");
        lints.append(&mut rule.lints().iter().map(|l| l.clone()).collect())
    }

    Json(ResultKind::Lints(lints))
}

fn main() {
    rocket::ignite()
        .mount("/", routes![lint, index, examples])
        .mount("/mwlint", routes![lint, index, examples])
        .launch();
}
