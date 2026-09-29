//! Model-visible wiki-only fallback: bounded ranked/literal/regex lookup and full-page windows.
use crate::{
    error::{Error, Result},
    memory::Pages,
    retrieval::Search,
};
use regex::RegexBuilder;
use serde::Deserialize;
use serde_json::{Value, json};
use std::time::{Duration, Instant};

pub const MAX_CALLS: usize = 3;

pub struct Lookup<'a> {
    pub search: &'a Search,
    pub pages: &'a Pages,
    pub calls: usize,
    pub sources: Vec<String>,
}

/// Tools accept only wiki queries or numeric revision IDs, never paths or commands.
pub fn definitions() -> Value {
    json!([
        {"type":"function","name":"wiki_search","description":"Fallback search of the complete in-RAM Minecraft Wiki. Use only when supplied passages miss evidence. ranked = BM25; literal or regex = case-insensitive full-text matching, like rg. Returns at most 8 excerpts with revision IDs and character offsets for wiki_read. For literal/regex, resume incomplete scans with next_cursor as cursor; omit cursor initially. Maximum 3 total tool calls per answer.","inputSchema":{"type":"object","properties":{"query":{"type":"string"},"mode":{"type":"string","enum":["ranked","literal","regex"]},"cursor":{"type":"integer"}},"required":["query","mode"],"additionalProperties":false}},
        {"type":"function","name":"wiki_read","description":"Read up to 6000 characters from a complete local wiki revision, including templates. Offset is a Unicode character position. Follow next_offset for another window only if necessary. No filesystem access. Maximum 3 total tool calls per answer.","inputSchema":{"type":"object","properties":{"revision_id":{"type":"integer"},"offset":{"type":"integer"}},"required":["revision_id","offset"],"additionalProperties":false}}
    ])
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Mode {
    Ranked,
    Literal,
    Regex,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SearchArgs {
    query: String,
    mode: Mode,
    #[serde(default)]
    cursor: usize,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReadArgs {
    revision_id: u64,
    offset: usize,
}

impl Lookup<'_> {
    pub fn call(&mut self, tool: &str, arguments: Value) -> Result<Value> {
        if self.calls >= MAX_CALLS {
            return Err(Error::Invalid(
                "wiki lookup budget exhausted; answer from available evidence".into(),
            ));
        }
        self.calls += 1;
        let result = match tool {
            "wiki_search" => self.search(serde_json::from_value(arguments)?),
            "wiki_read" => self.read(serde_json::from_value(arguments)?),
            _ => Err(Error::Invalid("unknown wiki tool".into())),
        }?;
        if serde_json::to_vec(&result)?.len() > 48 * 1024 {
            return Err(Error::Invalid("wiki tool result too large".into()));
        }
        Ok(result)
    }

    fn search(&mut self, args: SearchArgs) -> Result<Value> {
        if args.query.trim().is_empty() || args.query.chars().count() > 160 {
            return Err(Error::Invalid("query must contain 1-160 characters".into()));
        }
        if matches!(args.mode, Mode::Ranked) {
            if args.cursor != 0 {
                return Err(Error::Invalid("ranked search does not use a cursor".into()));
            }
            let result = self.search.search(&args.query)?;
            for hit in &result.hits {
                self.source(hit.url.clone());
            }
            return Ok(serde_json::to_value(result)?);
        }
        let pattern = match args.mode {
            Mode::Literal => regex::escape(&args.query),
            Mode::Regex => args.query,
            Mode::Ranked => return Err(Error::Invalid("invalid search dispatch".into())),
        };
        let regex = RegexBuilder::new(&pattern)
            .case_insensitive(true)
            .size_limit(1024 * 1024)
            .dfa_size_limit(4 * 1024 * 1024)
            .build()
            .map_err(|_| Error::Invalid("invalid or excessively complex regex".into()))?;
        let started = Instant::now();
        let mut hits = Vec::new();
        if args.cursor > self.pages.pages.len() {
            return Err(Error::Invalid("search cursor exceeds snapshot".into()));
        }
        let mut next_cursor = None;
        for (position, page) in self.pages.pages.iter().enumerate().skip(args.cursor) {
            if started.elapsed() > Duration::from_millis(250) {
                next_cursor = Some(position);
                break;
            }
            let found = if regex.is_match(&page.title) {
                Some(0)
            } else {
                regex.find(&page.text).map(|found| found.start())
            };
            if let Some(byte) = found {
                let offset = page.text[..byte].chars().count().saturating_sub(160);
                let text: String = page.text.chars().skip(offset).take(700).collect();
                let url = format!(
                    "https://minecraft.wiki/w/Special:Redirect/revision/{}",
                    page.revision_id
                );
                hits.push(json!({"title":page.title,"revision_id":page.revision_id,"offset":offset,"text":text,"url":url}));
                if hits.len() == 8 {
                    next_cursor = (position + 1 < self.pages.pages.len()).then_some(position + 1);
                    break;
                }
            }
        }
        for hit in &hits {
            if let Some(url) = hit["url"].as_str() {
                self.source(url.into());
            }
        }
        Ok(
            json!({"hits":hits,"truncated":next_cursor.is_some(),"next_cursor":next_cursor,"elapsed_micros":started.elapsed().as_micros()}),
        )
    }

    fn read(&mut self, args: ReadArgs) -> Result<Value> {
        if args.offset > 2_000_000 {
            return Err(Error::Invalid("page offset exceeds bound".into()));
        }
        let page = self
            .pages
            .revision(args.revision_id)
            .ok_or_else(|| Error::Invalid("unknown wiki revision".into()))?;
        let text: String = page.text.chars().skip(args.offset).take(6000).collect();
        let next = args.offset + text.chars().count();
        let next_offset = (next < page.text.chars().count()).then_some(next);
        let url = format!(
            "https://minecraft.wiki/w/Special:Redirect/revision/{}",
            page.revision_id
        );
        let result = json!({"title":page.title,"revision_id":page.revision_id,"offset":args.offset,"text":text,"next_offset":next_offset,"url":url});
        self.source(url);
        Ok(result)
    }

    fn source(&mut self, url: String) {
        if self.sources.len() < 8 && !self.sources.contains(&url) {
            self.sources.push(url);
        }
    }
}
