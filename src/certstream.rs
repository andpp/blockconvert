use crate::{Domain, EXTRACTED_DOMAINS_DIR};

use async_std::fs::OpenOptions;
use async_std::io::BufWriter;
use async_std::prelude::*;
use futures::StreamExt;

const URL: &str = "wss://certstream.calidog.io/";

pub async fn certstream() -> Result<(), Box<dyn std::error::Error>> {
    let _ = std::fs::create_dir(EXTRACTED_DOMAINS_DIR);
    let mut path = std::path::PathBuf::from(EXTRACTED_DOMAINS_DIR);
    path.push(std::path::PathBuf::from(format!(
        "certstream_{:?}",
        chrono::Utc::today()
    )));
    let mut wtr = BufWriter::new(
        OpenOptions::new()
            .append(true)
            .create(true)
            .open(path)
            .await?,
    );
    let mut counter: u64 = 0;
    let now = std::time::Instant::now();
    loop {
        let (mut ws_stream, _response) = tokio_tungstenite::connect_async(URL).await?;
        while let Some(Ok(next)) = ws_stream.next().await {
            if let tokio_tungstenite::tungstenite::protocol::Message::Text(data) = next {
                match serde_json::from_str::<serde_json::Value>(&data) {
                    Ok(decoded) => {
                        if let Some(all_domains) = decoded
                            .pointer("/data/leaf_cert/all_domains")
                            .and_then(|data| data.as_array())
                        {
                            for domain in all_domains
                                .into_iter()
                                .filter_map(|domain| domain.as_str())
                                .filter_map(|domain| domain.parse::<Domain>().ok())
                            {
                                if counter % 10000 == 0 {
                                    println!(
                                        "Found {} domains ({}/s) via CertStream. Current domain: {}",
                                        counter, (counter as f32 / now.elapsed().as_secs_f32()), domain
                                    );
                                }
                                counter += 1;
                                wtr.write_all(domain.as_bytes()).await?;
                                wtr.write_all(b"\n").await?;
                            }
                        } else {
                            println!("Failed to extract `all_domain`");
                        }
                    }
                    Err(error) => {
                        println!("Error: {:?}", error);
                        println!("Failed to decode: {:?}\n", data);
                    }
                }
            } else {
                println!("Unknown type: {:?}", next)
            }
        }
    }
}
