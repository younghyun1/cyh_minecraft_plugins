//! CLI orchestration and chronology reconstruction.
use crate::{
    error::Error,
    input,
    parser::{event, state::Report},
};
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use clap::Parser;
use std::{
    io::{self, Write},
    path::PathBuf,
};

#[derive(Parser)]
#[command(
    version,
    about = "Stream PaperMC logs into a JSON player/playtime report"
)]
struct Args {
    /// Log directories or dated .log/.log.gz files (default: ../logs).
    #[arg(default_value = "../logs")]
    paths: Vec<PathBuf>,
    /// First day of latest.log; otherwise its local modification date is used.
    #[arg(long)]
    latest_date: Option<NaiveDate>,
    /// Maximum distinct player names retained in memory.
    #[arg(long, default_value_t = 100_000, value_parser = clap::value_parser!(u32).range(1..))]
    max_players: u32,
    /// Indent the JSON report.
    #[arg(long)]
    pretty: bool,
}

/// Advance at midnight; small backwards clock movements are skipped and counted.
fn timestamp(
    date: &mut NaiveDate,
    previous: &mut Option<NaiveTime>,
    time: NaiveTime,
) -> Result<NaiveDateTime, Error> {
    if let Some(last) = *previous
        && (last - time).num_seconds() > 12 * 60 * 60
    {
        *date = match date.succ_opt() {
            Some(date) => date,
            None => return Err(Error::DateOverflow),
        };
    }
    *previous = Some(time);
    Ok(date.and_time(time))
}

/// Keep memory proportional to players, with no retained log lines or session history.
pub fn run() -> Result<(), Error> {
    let args = Args::parse();
    let inputs = input::discover(&args.paths, args.latest_date)?;
    let mut report = Report::default();
    let mut buffer = String::with_capacity(4096);
    for input in inputs {
        let mut reader = input::reader(&input)?;
        let mut date = input.date;
        let mut previous = None;
        let mut first = true;
        report.files += 1;
        while input::line(&mut *reader, &mut buffer)? {
            report.lines += 1;
            let (time, event) = match event::parse(buffer.trim_end_matches(['\r', '\n'])) {
                Some(parsed) => parsed,
                None => {
                    report.ignored_lines += 1;
                    continue;
                }
            };
            let at = timestamp(&mut date, &mut previous, time)?;
            if let Some(last) = report.observed_until {
                if at < last {
                    report.out_of_order_lines += 1;
                    continue;
                }
                // Missing whole calendar days mean continuity cannot be established.
                if first && (at.date() - last.date()).num_days() > 1 {
                    report.boundary(last, false);
                }
            }
            first = false;
            report.apply(at, event, args.max_players as usize)?;
        }
    }
    report.finish();
    let mut output = io::BufWriter::new(io::stdout().lock());
    if args.pretty {
        serde_json::to_writer_pretty(&mut output, &report)?;
    } else {
        serde_json::to_writer(&mut output, &report)?;
    }
    writeln!(output)?;
    output.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Midnight is a date transition; minor reordering is not a whole extra day.
    #[test]
    fn midnight() -> Result<(), Box<dyn std::error::Error>> {
        let mut date = NaiveDate::parse_from_str("2026-09-27", "%Y-%m-%d")?;
        let mut previous = None;
        let start = timestamp(
            &mut date,
            &mut previous,
            NaiveTime::parse_from_str("23:59:50", "%H:%M:%S")?,
        )?;
        let end = timestamp(
            &mut date,
            &mut previous,
            NaiveTime::parse_from_str("00:00:10", "%H:%M:%S")?,
        )?;
        assert_eq!((end - start).num_seconds(), 20);
        let back = timestamp(
            &mut date,
            &mut previous,
            NaiveTime::parse_from_str("00:00:09", "%H:%M:%S")?,
        )?;
        assert_eq!((end - back).num_seconds(), 1);
        Ok(())
    }
}
