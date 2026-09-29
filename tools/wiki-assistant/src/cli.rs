//! Explicit commands keep network acquisition out of runtime chat handling.
use crate::{error::Result, retrieval::Search};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(about, version)]
pub struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Download/resume all English documentation and transclusion sources.
    Download {
        #[arg(long)]
        corpus: PathBuf,
    },
    /// Build an immutable index from a completed snapshot into a new directory.
    Index {
        #[arg(long)]
        corpus: PathBuf,
        #[arg(long)]
        index: PathBuf,
    },
    /// Measure complete wiki text, archive, and index sizes without inference.
    Stats {
        #[arg(long)]
        corpus: PathBuf,
        #[arg(long)]
        index: PathBuf,
    },
    /// Run one local lookup; no Codex process or network access.
    Search {
        #[arg(long)]
        index: PathBuf,
        question: String,
        /// Repeat against one open index to measure warm retrieval separately from startup.
        #[arg(long, default_value_t = 1, value_parser = clap::value_parser!(u16).range(1..=1000))]
        repeat: u16,
    },
    /// Serve sequential private chat requests on stdin/stdout.
    Serve {
        #[arg(long)]
        corpus: PathBuf,
        #[arg(long)]
        index: PathBuf,
        #[arg(long)]
        codex: PathBuf,
        #[arg(long)]
        codex_home: PathBuf,
        #[arg(long)]
        work_dir: PathBuf,
    },
}

impl Cli {
    /// Blocking import and indexing never run inside a Tokio worker.
    pub fn run(self) -> Result<()> {
        match self.command {
            Command::Download { corpus } => crate::download::download(&corpus),
            Command::Index { corpus, index } => crate::index::build(&corpus, &index),
            Command::Stats { corpus, index } => crate::corpus::stats(&corpus, &index),
            Command::Search {
                index,
                question,
                repeat,
            } => {
                let search = Search::open(&index)?;
                let mut result = search.search(&question)?;
                let cold = result.retrieval_micros;
                let mut times = vec![cold];
                for _ in 1..repeat {
                    result = search.search(&question)?;
                    times.push(result.retrieval_micros);
                }
                times.sort_unstable();
                println!(
                    "{}",
                    serde_json::to_string(
                        &serde_json::json!({"hits":result.hits,"samples":repeat,"first_micros":cold,
                    "p50_micros":times[times.len()/2],"p95_micros":times[(times.len()-1)*95/100]})
                    )?
                );
                Ok(())
            }
            Command::Serve {
                corpus,
                index,
                codex,
                codex_home,
                work_dir,
            } => {
                let search = Search::open(&index)?;
                search.verify_corpus(&corpus)?;
                let pages = crate::memory::Pages::load(&corpus)?;
                tracing::info!(
                    pages = pages.pages.len(),
                    text_bytes = pages.text_bytes,
                    "Wiki loaded into RAM"
                );
                tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(2)
                    .enable_all()
                    .build()?
                    .block_on(crate::bridge::serve(
                        search,
                        pages,
                        &codex,
                        &codex_home,
                        &work_dir,
                    ))
            }
        }
    }
}
