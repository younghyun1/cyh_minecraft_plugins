//! Single-request process framing with bounded allocation and redacted failures.

use std::io::{Read, Write};

use crate::{Error, MAX_REQUEST_BYTES, MAX_RESPONSE_BYTES, PredictionRequest, predict};

/// Read one newline-terminated request through EOF and write one response line.
/// The caller must close stdin after its frame and enforce a process deadline.
pub fn run(input: impl Read, mut output: impl Write) -> Result<(), Error> {
    let mut bytes = Vec::new();
    input
        .take((MAX_REQUEST_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| Error::Io)?;
    if bytes.len() > MAX_REQUEST_BYTES || bytes.last() != Some(&b'\n') {
        return Err(Error::Frame);
    }
    let request: PredictionRequest = serde_json::from_slice(&bytes).map_err(|_| Error::Json)?;
    let response = predict(&request)?;
    let mut encoded = serde_json::to_vec(&response).map_err(|_| Error::Encoding)?;
    if encoded.len() >= MAX_RESPONSE_BYTES {
        return Err(Error::Encoding);
    }
    encoded.push(b'\n');
    output.write_all(&encoded).map_err(|_| Error::Io)?;
    output.flush().map_err(|_| Error::Io)
}

#[cfg(test)]
mod tests {
    use super::run;
    use crate::{Error, GENERATOR_REVISION, MAX_REQUEST_BYTES, PredictionResponse};

    const REQUEST: &str = r#"{"seed":1,"large_biomes":false,"y":64,"min_x":176,"min_z":148,"width":1,"height":1,"step":4}"#;

    /// Malformed, extra and incomplete inputs must fail without partial output.
    #[test]
    fn rejects_unframed_unknown_duplicate_and_oversized_inputs() {
        for input in [
            REQUEST.to_owned(),
            format!("{REQUEST}\n{REQUEST}\n"),
            format!("{{\"seed\":2,{}\n", &REQUEST[1..]),
            format!("{{\"secret\":123456789,{}\n", &REQUEST[1..]),
            format!("{}\n", REQUEST.replace("\"width\":1", "\"width\":1.5")),
        ] {
            let mut output = Vec::new();
            assert!(run(input.as_bytes(), &mut output).is_err());
            assert!(output.is_empty());
        }
        let input = vec![b' '; MAX_REQUEST_BYTES + 1];
        assert!(matches!(
            run(input.as_slice(), Vec::new()),
            Err(Error::Frame)
        ));
    }

    /// Successful frames expose only public cells and generator identity.
    #[test]
    fn round_trip_has_one_bounded_response_and_no_seed() -> Result<(), Box<dyn std::error::Error>> {
        let mut input = format!("{REQUEST}\n");
        input.insert_str(0, &" ".repeat(MAX_REQUEST_BYTES - input.len()));
        let mut output = Vec::new();
        run(input.as_bytes(), &mut output)?;
        assert_eq!(output.iter().filter(|byte| **byte == b'\n').count(), 1);
        let response: PredictionResponse = serde_json::from_slice(&output)?;
        assert_eq!(response.generator_revision, GENERATOR_REVISION);
        assert!(!response.large_biomes);
        assert_eq!(response.cells.len(), 1);
        let value: serde_json::Value = serde_json::from_slice(&output)?;
        assert!(value.get("seed").is_none());
        Ok(())
    }
}
