#![allow(dead_code)]

use std::{io::Write, sync::Mutex};

use launcher_core::{application::ports::outbound::ProgressReporter, domain::Progress};
use launcher_infrastructure::HttpClient;
use sha1::{Digest, Sha1};

pub fn http() -> HttpClient {
    HttpClient::new("test").unwrap()
}

pub fn sha1_hex(bytes: &[u8]) -> String {
    hex::encode(Sha1::digest(bytes))
}

#[derive(Default)]
pub struct RecordingReporter {
    pub events: Mutex<Vec<Progress>>,
}

impl RecordingReporter {
    pub fn last(&self) -> Option<Progress> {
        self.events.lock().unwrap().last().cloned()
    }
}

impl ProgressReporter for RecordingReporter {
    fn report(&self, progress: Progress) {
        self.events.lock().unwrap().push(progress);
    }
}

/// Builds an in-memory zip from `(name, contents)` pairs.
pub fn zip_bytes(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default();
    for (name, contents) in entries {
        writer.start_file(*name, options).unwrap();
        writer.write_all(contents).unwrap();
    }
    writer.finish().unwrap().into_inner()
}
