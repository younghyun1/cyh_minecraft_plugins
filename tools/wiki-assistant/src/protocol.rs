//! Bounded framing and plain-text output shared by bridge tests.
use crate::error::{Error, Result};
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncReadExt};

/// Send a bounded startup failure on the existing pipe instead of losing stderr diagnostics.
pub fn startup_failure(error: &Error) -> Result<()> {
    use std::io::Write;
    let frame =
        serde_json::json!({"ready":false,"protocol":1,"diagnostic":error.startup_diagnostic()});
    let mut output = std::io::stdout().lock();
    writeln!(output, "{frame}")?;
    output.flush()?;
    Ok(())
}

/// Take caps allocations even if a child never sends a newline.
pub async fn line<R: AsyncBufRead + Unpin>(reader: &mut R, max: usize) -> Result<String> {
    let mut bytes = Vec::new();
    let count = reader
        .take((max + 1) as u64)
        .read_until(b'\n', &mut bytes)
        .await?;
    if count == 0 || count > max || bytes.last() != Some(&b'\n') {
        return Err(Error::Invalid(
            "truncated or oversized protocol frame".into(),
        ));
    }
    String::from_utf8(bytes).map_err(|_| Error::Invalid("non-UTF8 frame".into()))
}

/// Hard cap the display contract even if inference ignores the requested style.
pub fn answer(raw: &str) -> String {
    let plain: String = raw
        .chars()
        .filter(|c| !c.is_control() || c.is_whitespace())
        .filter(|c| *c != '§' && !matches!(*c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}'))
        .collect();
    let text = plain
        .split_whitespace()
        .take(60)
        .collect::<Vec<_>>()
        .join(" ");
    if text.chars().count() <= 360 {
        text
    } else {
        text.chars().take(359).chain(['…']).collect()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn answers_are_short_plain_unicode_paragraphs() {
        let result = super::answer(&format!("§\u{202e}hello\n{}", "🪨 word ".repeat(100)));
        assert!(!result.contains(['§', '\n', '\u{202e}']));
        assert!(result.chars().count() <= 360);
        assert!(result.split_whitespace().count() <= 60);
    }
    #[tokio::test]
    async fn rejects_oversized_and_unterminated_frames() {
        assert!(super::line(&mut &b"12345\n"[..], 5).await.is_err());
        assert!(super::line(&mut &b"123"[..], 5).await.is_err());
        assert!(super::line(&mut &b"ok\n"[..], 5).await.is_ok());
    }
}
