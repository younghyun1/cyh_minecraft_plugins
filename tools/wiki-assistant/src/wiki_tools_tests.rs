//! Full resident lookup, bounded regex, and Unicode page-window contracts.
use crate::{error::Result, fixtures, memory, wiki_tools::Lookup};
use serde_json::json;

#[test]
fn queries_and_full_pages_survive_source_removal() -> Result<()> {
    let (search, pages) = fixtures::resident()?;
    assert!(search.search("Pail")?.hits[0].title.starts_with("Bucket"));
    let mut lookup = Lookup {
        search: &search,
        pages: &pages,
        calls: 0,
        sources: Vec::new(),
    };
    let result = lookup.call(
        "wiki_search",
        json!({"query":"secret recipe","mode":"literal"}),
    )?;
    assert_eq!(result["hits"][0]["revision_id"], 102);
    let result = lookup.call(
        "wiki_search",
        json!({"query":"^Template:HiddenRecipe$","mode":"regex"}),
    )?;
    assert_eq!(result["hits"][0]["revision_id"], 102);
    let result = lookup.call("wiki_read", json!({"revision_id":102,"offset":2}))?;
    assert!(
        result["text"]
            .as_str()
            .is_some_and(|text| text.starts_with("secret recipe") && text.chars().count() == 6000)
    );
    assert_eq!(result["next_offset"], 6002);
    assert!(
        lookup
            .call("wiki_read", json!({"revision_id":102,"offset":6002}))
            .is_err()
    );
    assert_eq!(lookup.sources.len(), 1);
    Ok(())
}

#[test]
fn malformed_and_excessive_requests_are_bounded() -> Result<()> {
    let (search, pages) = fixtures::resident()?;
    for (tool, arguments) in [
        ("wiki_search", json!({"query":"(","mode":"regex"})),
        (
            "wiki_search",
            json!({"query":"a".repeat(161),"mode":"literal"}),
        ),
        ("wiki_search", json!({"query":" ","mode":"ranked"})),
        ("wiki_search", json!({"query":"a","mode":"shell"})),
        (
            "wiki_search",
            json!({"query":"a","mode":"literal","cursor":100001}),
        ),
        (
            "wiki_search",
            json!({"query":"a","mode":"ranked","cursor":1}),
        ),
        ("wiki_read", json!({"revision_id":999,"offset":0})),
        ("wiki_read", json!({"revision_id":102,"offset":2000001})),
        (
            "wiki_read",
            json!({"revision_id":102,"offset":0,"path":"/etc/passwd"}),
        ),
        ("shell", json!({"command":"anything"})),
    ] {
        let mut lookup = Lookup {
            search: &search,
            pages: &pages,
            calls: 0,
            sources: Vec::new(),
        };
        assert!(lookup.call(tool, arguments).is_err());
        assert_eq!(lookup.calls, 1);
    }
    Ok(())
}

#[test]
fn scan_cursor_skips_prior_pages_and_reports_completion() -> Result<()> {
    let (search, pages) = fixtures::resident()?;
    let mut lookup = Lookup {
        search: &search,
        pages: &pages,
        calls: 0,
        sources: Vec::new(),
    };
    let result = lookup.call(
        "wiki_search",
        json!({"query":".","mode":"regex","cursor":2}),
    )?;
    assert_eq!(result["hits"].as_array().map(Vec::len), Some(1));
    assert_eq!(result["hits"][0]["revision_id"], 102);
    assert_eq!(result["truncated"], false);
    assert!(result["next_cursor"].is_null());
    Ok(())
}

#[test]
fn resident_index_rejects_oversized_and_symlinked_files() -> Result<()> {
    let root = tempfile::tempdir()?;
    let file = root.path().join("oversized");
    std::fs::File::create(&file)?.set_len(memory::MAX_INDEX_BYTES + 1)?;
    assert!(memory::index(root.path()).is_err());
    std::fs::remove_file(file)?;
    std::os::unix::fs::symlink("/missing-file", root.path().join("link"))?;
    assert!(memory::index(root.path()).is_err());
    Ok(())
}
