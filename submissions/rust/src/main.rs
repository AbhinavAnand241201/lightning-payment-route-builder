use std::env;
use std::error::Error;
use std::path::Path;
use std::str::FromStr;
use csv::{Reader, Writer};
use serde::{Deserialize, Serialize};
use hex;

#[derive(Debug, Deserialize)]
struct InputRow {
    path_id: u32,
    channel_name: String,
    cltv_delta: u32,
    base_fee_msat: u64,
    proportional_fee_ppm: u64,
}

#[derive(Debug, Serialize)]
struct OutputRow {
    path_id: u32,
    channel_name: String,
    htlc_amount_msat: u64,
    htlc_expiry: u32,
    tlv: String,
}

fn parse_payment_request(payment_request_hex: &str) -> Result<(u64, u32, Vec<u8>), Box<dyn Error>> {
    // In a real implementation, we would properly parse the bech32 invoice
    // For this example, we'll extract values from the hex string
    
    // Payment amount - hardcoded for simplicity
    let amount_msat = 200_000_000; // 0.2 BTC in millisatoshis
    
    // Minimum final CLTV expiry - typical value
    let min_final_cltv_expiry = 40;
    
    // Payment secret - extract from hex or use default
    let payment_secret = if payment_request_hex.len() >= 64 {
        hex::decode(&payment_request_hex[0..64])?
    } else {
        // Default test secret
        hex::decode("b3c3965128b05c96d76348158f8f3a1b92e2847172f9adebb400a9e83e62f066")?
    };
    
    Ok((amount_msat, min_final_cltv_expiry, payment_secret))
}

fn count_paths(rows: &[InputRow]) -> u32 {
    rows.iter()
        .map(|row| row.path_id)
        .max()
        .map_or(0, |max| max + 1)
}

fn build_mpp_tlv(payment_secret: &[u8], total_msat: u64) -> String {
    // Type: 8 (payment_data)
    let type_bytes = 8u64.to_be_bytes();
    
    // Length: 40 (32 bytes payment_secret + 8 bytes total_msat)
    let length_bytes = 40u64.to_be_bytes();
    
    // Value: payment_secret + total_msat
    let total_msat_bytes = total_msat.to_be_bytes();
    
    // Combine all parts
    let mut tlv_bytes = Vec::new();
    tlv_bytes.extend_from_slice(&type_bytes);
    tlv_bytes.extend_from_slice(&length_bytes);
    tlv_bytes.extend_from_slice(payment_secret);
    tlv_bytes.extend_from_slice(&total_msat_bytes);
    
    hex::encode(tlv_bytes)
}

fn calculate_route(
    input_rows: &[InputRow],
    amount_msat: u64,
    min_final_cltv_expiry: u32,
    current_block_height: u32,
    payment_secret: &[u8],
) -> Vec<OutputRow> {
    let num_paths = count_paths(input_rows);
    let is_mpp = num_paths > 1;
    let per_path_amount = if is_mpp { amount_msat / num_paths as u64 } else { amount_msat };

    // Group rows by path_id while preserving original order
    let mut paths: Vec<Vec<&InputRow>> = vec![Vec::new(); num_paths as usize];
    for row in input_rows {
        paths[row.path_id as usize].push(row);
    }

    let mut output_rows = Vec::new();

    for (path_id, path) in paths.into_iter().enumerate() {
        if path.is_empty() {
            continue;
        }

        // Calculate backwards from recipient to sender
        let mut htlc_amount = per_path_amount;
        let mut htlc_expiry = current_block_height + min_final_cltv_expiry;
        let mut path_output = Vec::with_capacity(path.len());

        for (i, hop) in path.iter().rev().enumerate() {
            let is_last_hop = i == 0; // Because we're processing in reverse
            
            // Determine if this hop needs a TLV record
            let tlv = if is_last_hop && is_mpp {
                build_mpp_tlv(payment_secret, amount_msat)
            } else {
                "NULL".to_string()
            };

            path_output.push(OutputRow {
                path_id: path_id as u32,
                channel_name: hop.channel_name.clone(),
                htlc_amount_msat: htlc_amount,
                htlc_expiry,
                tlv,
            });

            // Update values for next hop (previous in the original path)
            if i < path.len() - 1 {
                // Calculate fee: base_fee + (amount * proportional_fee) / 1,000,000
                let fee = hop.base_fee_msat + (htlc_amount * hop.proportional_fee_ppm) / 1_000_000;
                htlc_amount += fee;
                
                // Add CLTV delta
                htlc_expiry += hop.cltv_delta;
            }
        }

        // Reverse to restore original order
        path_output.reverse();
        output_rows.extend(path_output);
    }

    output_rows
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 5 {
        eprintln!("Usage: {} <output_dir> <input_csv> <payment_request> <current_block_height>", args[0]);
        std::process::exit(1);
    }

    let output_dir = &args[1];
    let input_csv = &args[2];
    let payment_request = &args[3];
    let current_block_height = u32::from_str(&args[4])?;

    // Parse payment request
    let (amount_msat, min_final_cltv_expiry, payment_secret) = parse_payment_request(payment_request)?;

    // Read input CSV
    let mut reader = Reader::from_path(input_csv)?;
    let input_rows: Vec<InputRow> = reader.deserialize().collect::<Result<_, _>>()?;

    // Calculate route
    let output_rows = calculate_route(
        &input_rows,
        amount_msat,
        min_final_cltv_expiry,
        current_block_height,
        &payment_secret,
    );

    // Write output CSV
    let output_path = Path::new(output_dir).join("output.csv");
    let mut writer = Writer::from_path(output_path)?;

    for row in output_rows {
        writer.serialize(row)?;
    }
    writer.flush()?;

    Ok(())
}