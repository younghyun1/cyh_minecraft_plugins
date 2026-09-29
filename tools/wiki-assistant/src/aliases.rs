//! Match complete article titles and redirect names in the same ranked query.
use crate::{corpus, error::Result, index};
use std::{collections::BTreeMap, path::Path};

pub fn key(text: &str) -> String {
    let mut analyzer = index::analyzer();
    let mut stream = analyzer.token_stream(text);
    let mut words = Vec::new();
    while stream.advance() {
        words.push(stream.token().text.clone());
        if words.len() == 24 {
            break;
        }
    }
    words.join(" ")
}

pub fn target(text: &str) -> Option<String> {
    let prefix: String = text.trim_start().chars().take(512).collect();
    if !prefix.to_ascii_lowercase().starts_with("#redirect") {
        return None;
    }
    let (_, linked) = prefix.split_once("[[")?;
    let (title, _) = linked.split_once("]]")?;
    Some(title.split(['#', '|']).next()?.trim().replace('_', " "))
}

/// Fixed snapshot metadata is dropped after indexing; ordered maps make alias selection reproducible.
pub fn load(root: &Path, batches: u64) -> Result<BTreeMap<String, Vec<String>>> {
    let mut redirects = BTreeMap::new();
    for batch in 0..batches {
        for page in corpus::read_batch(root, batch)? {
            if let Some(target) = target(&page.text) {
                redirects.insert(page.title, target);
            }
        }
    }
    let mut aliases: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (alias, target) in &redirects {
        let mut resolved = target;
        for _ in 0..8 {
            match redirects.get(resolved) {
                Some(next) => resolved = next,
                None => break,
            }
        }
        let names = aliases.entry(resolved.clone()).or_default();
        if names.len() < 64 {
            names.push(key(alias));
        }
    }
    Ok(aliases)
}

#[cfg(test)]
mod tests {
    #[test]
    fn redirect_and_article_keys_normalize() {
        assert_eq!(
            super::target("#REDIRECT [[Diamond_Ore#Mining]]"),
            Some("Diamond Ore".into())
        );
        assert_eq!(super::target("A diamond is an item."), None);
        assert_eq!(super::key("Diamonds"), super::key("Diamond"));
    }
}
