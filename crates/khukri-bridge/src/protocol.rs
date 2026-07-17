use std::collections::HashMap;
use std::io::{self, Read, Write};

use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum Incoming {
    #[serde(rename = "queue_download")]
    QueueDownload {
        url: String,
        filename: Option<String>,
        size: Option<u64>,
        quality: Option<String>,
        source: Option<String>,
        #[serde(rename = "pageUrl")]
        page_url: Option<String>,
        #[serde(rename = "customHeaders", default)]
        custom_headers: HashMap<String, String>,
    },
}

#[derive(Debug, Serialize)]
pub struct BridgeEvent {
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub id: String,
    pub status: &'static str,
    pub bytes_done: u64,
    pub total_bytes: Option<u64>,
    pub speed_bps: u64,
    pub eta_seconds: Option<u64>,
    pub segments_done: u32,
    pub segments_total: Option<u32>,
    pub source: Option<String>,
    pub output_path: Option<String>,
    pub message: Option<String>,
}

pub fn read_message() -> Result<Incoming> {
    let mut len_buf = [0u8; 4];
    io::stdin().read_exact(&mut len_buf)?;
    let len = u32::from_le_bytes(len_buf) as usize;
    let mut buf = vec![0u8; len];
    io::stdin().read_exact(&mut buf)?;
    Ok(serde_json::from_slice(&buf)?)
}

pub fn write_message<T: Serialize>(writer: &mut impl Write, msg: &T) -> Result<()> {
    let body = serde_json::to_vec(msg)?;
    let len = (body.len() as u32).to_le_bytes();
    writer.write_all(&len)?;
    writer.write_all(&body)?;
    writer.flush()?;
    Ok(())
}
