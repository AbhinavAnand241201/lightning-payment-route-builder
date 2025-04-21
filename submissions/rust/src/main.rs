use csv::{Reader, WriterBuilder};
use lightning_invoice::Bolt11Invoice;
use serde::{Deserialize, Serialize};
use std::env;
use std::path::Path;
use std::str::FromStr;

// hop in the payment route
#[derive(Debug, Deserialize, PartialEq)]
struct RouteHop {
    path_id: u32,
    channel_name: String,
    cltv_delta: u32,
    base_fee_msat: u64,
    proportional_fee_ppm: u64,
}

// calculate the HTLC values for a hop
#[derive(Debug, Serialize)]
struct HtlcOutput {
    path_id: u32,
    channel_name: String,
    htlc_amount_msat: u64,
    htlc_expiry: u32,
    tlv: String,
}

// calculate the fee for forwarding an htlC
fn calculate_fee(amount_msat: u64, base_fee_msat: u64, proportional_fee_ppm: u64) -> u64 {
    base_fee_msat + (amount_msat * proportional_fee_ppm) / 1_000_000
}

// create the tlv record
fn create_mpp_tlv(payment_secret: &[u8], total_msat: u64) -> String {
    let mut tlv = Vec::new();

    // type (8) - 8 bytes
    tlv.extend_from_slice(&8u64.to_be_bytes());

    // length (40) - 8 bytes
    tlv.extend_from_slice(&40u64.to_be_bytes());

    // payment secret - 32 bytes
    tlv.extend_from_slice(payment_secret);

    // total amount in millisatoshis - 8 bytes
    tlv.extend_from_slice(&total_msat.to_be_bytes());

    // convert to hex string
    tlv.iter().map(|b| format!("{:02x}", b)).collect::<String>()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse command line arguments
    let args: Vec<String> = env::args().collect();
    if args.len() != 5 {
        // here the first arg is the cli itself
        eprintln!(
            "Usage: {} <output_dir> <input_csv> <payment_request> <block_height>",
            args[0]
        );
        std::process::exit(1);
    }

    let output_dir = &args[1];
    let input_csv = &args[2];
    let payment_request = &args[3];
    let current_height: u32 = args[4].parse()?;

    // parse the payment invoice
    let invoice = Bolt11Invoice::from_str(payment_request)
        .map_err(|e| format!("Failed to parse invoice: {:?}", e))?;
    let payment_amount_msat = invoice
        .amount_milli_satoshis()
        .ok_or("Payment request must specify an amount")?;

    // retriev min final cltv from invoice
    let min_final_cltv_delta = invoice.min_final_cltv_expiry_delta() as u32;

    // read the input file
    let mut rdr = Reader::from_path(input_csv)?;
    let hops: Vec<RouteHop> = rdr.deserialize().collect::<Result<_, _>>()?;

    let mut paths: std::collections::HashMap<u32, Vec<RouteHop>> = std::collections::HashMap::new();
    for hop in hops {
        paths.entry(hop.path_id).or_insert_with(Vec::new).push(hop);
    }

    let path_count = paths.len() as u64;
    let base_amount_per_path = payment_amount_msat / path_count;

    // let's create the output file
    let output_path = Path::new(output_dir).join("output.csv");
    let mut wtr = WriterBuilder::new()
        .has_headers(false)
        .from_path(output_path)?;

    let mut all_htlc_outputs: Vec<HtlcOutput> = Vec::new();

    // process each path
    for (path_id, path_hops) in paths.iter() {
        let mut current_amount = base_amount_per_path;
        let mut current_expiry = current_height;

        // calculate fees and amounts backwards
        let mut amounts_and_expiries: Vec<(u64, u32)> = Vec::new();

        let mut prev_cltv_delta = 0;
        let mut prev_amount_msat = 0;
        let mut prev_proportional_fee_ppm = 0;

        for (index, hop) in path_hops.iter().rev().enumerate() {
            // for the final hop we need to add the min_final_cltv_delta
            let cltv_delta = if index == 0 {
                min_final_cltv_delta
            } else {
                prev_cltv_delta
            };
            prev_cltv_delta = hop.cltv_delta;

            current_expiry += cltv_delta;

            // calculate fee for intermediate hops
            if index > 0 {
                let fee =
                    calculate_fee(current_amount, prev_amount_msat, prev_proportional_fee_ppm);
                current_amount += fee;
            }

            prev_proportional_fee_ppm = hop.proportional_fee_ppm;

            prev_amount_msat = hop.base_fee_msat;

            amounts_and_expiries.push((current_amount, current_expiry));
        }
        amounts_and_expiries.reverse();

        // write hltc values for each hop
        for (i, (hop, (amount, expiry))) in path_hops.iter().zip(amounts_and_expiries).enumerate() {
            let tlv = if i == path_hops.len() - 1 && paths.len() > 1 {
                create_mpp_tlv(invoice.payment_secret().0.as_slice(), payment_amount_msat)
            } else {
                "NULL".to_string()
            };

            all_htlc_outputs.push(HtlcOutput {
                path_id: *path_id,
                channel_name: hop.channel_name.clone(),
                htlc_amount_msat: amount,
                htlc_expiry: expiry,
                tlv,
            });
        }
    }

    // Sort outputs to ensure consistent order
    all_htlc_outputs.sort_by_key(|output| (output.path_id, output.channel_name.clone()));

    // Write sorted outputs
    for output in all_htlc_outputs {
        wtr.serialize(output)?;
    }

    wtr.flush()?;
    Ok(())
}
