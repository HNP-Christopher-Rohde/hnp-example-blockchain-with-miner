use serde::{Serialize, Deserialize};
use sha2::{Sha256, Digest};
use std::time::{SystemTime, UNIX_EPOCH};
use reqwest;
use hex;
use anyhow::{Result, Context};
use tokio::task;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Block {
    pub index: u64,
    pub timestamp: u64,
    pub data: Vec<u8>,
    pub previous_hash: String,
    pub hash: String,
    pub nonce: u64,
}

impl Block {
    pub fn new(index: u64, data: Vec<u8>, previous_hash: String) -> Self {
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let mut block = Block {
            index,
            timestamp,
            data,
            previous_hash,
            hash: String::new(),
            nonce: 0,
        };
        block.hash = block.calculate_hash();
        block
    }

    pub fn calculate_hash(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(format!("{}{}{}{}{}", self.index, self.timestamp, hex::encode(&self.data), self.previous_hash, self.nonce));
        let result = hasher.finalize();
        hex::encode(result)
    }
}

fn meets_difficulty(hash: &str, difficulty: u32) -> bool {
    let target = vec![0u8; difficulty as usize];
    let hash_bytes = hex::decode(hash).expect("Hex decode failed");
    hash_bytes.starts_with(&target)
}

async fn mine_block(previous_block: &Block, data: Vec<u8>, difficulty: u32) -> Block {
    let mut new_block = Block::new(previous_block.index + 1, data.clone(), previous_block.hash.clone());

    // Parallel mining using multiple threads
    let mining_task = task::spawn_blocking(move || {
        let mut attempt = 0;
        while !meets_difficulty(&new_block.hash, difficulty) {
            new_block.nonce += 1;
            new_block.hash = new_block.calculate_hash();

            attempt += 1;
            if attempt % 100 == 0 {
                println!("Attempt {}: Trying hash: {}", attempt, new_block.hash);
            }
        }

        new_block
    });

    mining_task.await.unwrap()
}

async fn get_last_block_from_server() -> Result<Block> {
    let client = reqwest::Client::new();
    let url = "http://localhost:8000/last-block";

    let res = client.get(url)
        .send()
        .await
        .context("Failed to send request to get last block")?;

    let last_block: Block = res.json().await
        .context("Failed to parse last block")?;
    Ok(last_block)
}

async fn send_block_to_server(block: &Block) -> Result<()> {
    let client = reqwest::Client::new();
    let url = "http://localhost:8000/new-block";

    println!("Sending block: {:?}", block);

    let res = client.post(url)
        .json(block)
        .send()
        .await
        .context("Failed to send request to post new block")?;

    let status = res.status();
    let body = res.text().await
        .context("Failed to read response text")?;

    if status.is_success() {
        println!("Block successfully sent to server.");
    } else {
        println!("Failed to send block to server: {} - {}", status, body);
    }

    Ok(())
}

async fn get_difficulty_from_server() -> Result<u32> {
    let client = reqwest::Client::new();
    let url = "http://localhost:8000/difficulty";

    let res = client.get(url)
        .send()
        .await
        .context("Failed to send request to get difficulty")?;

    let difficulty_str = res.text().await
        .context("Failed to read difficulty response")?;
    let difficulty = difficulty_str.trim_start_matches("Difficulty: ")
        .parse::<u32>()
        .context("Failed to parse difficulty")?;

    Ok(difficulty)
}

async fn display_difficulty() -> Result<()> {
    let difficulty = get_difficulty_from_server().await
        .context("Error retrieving difficulty from server")?;

    println!("Current Difficulty: {}", difficulty);

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    loop {
        // Anzeige der aktuellen Schwierigkeit
        display_difficulty().await
            .context("Error displaying difficulty")?;

        // Hol die Schwierigkeit vom Server
        let difficulty = get_difficulty_from_server().await
            .context("Error retrieving difficulty from server")?;

        // Hol den letzten Block vom Server
        let previous_block = get_last_block_from_server().await
            .context("Error retrieving last block from server")?;

        let data = b"Block data".to_vec();
        let new_block = mine_block(&previous_block, data, difficulty).await;

        send_block_to_server(&new_block).await
            .context("Error sending block to server")?;

        // Warte eine gewisse Zeit, bevor der nächste Block erstellt wird
        tokio::time::sleep(tokio::time::Duration::from_secs(0)).await; // Wartezeit erhöht
    }
}
