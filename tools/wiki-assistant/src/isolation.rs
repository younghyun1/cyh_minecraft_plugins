//! Reject operator integrations while accepting files materialized by Codex itself.
use crate::error::{Error, Result};
use std::{fs, path::Path};

/// Codex installs bundled skills on first launch even when host skill discovery is disabled.
pub fn validate_home(home: &Path) -> Result<()> {
    for name in ["config.toml", "AGENTS.md", "plugins", "hooks.json"] {
        match fs::symlink_metadata(home.join(name)) {
            Ok(_) => {
                return Err(Error::Invalid(format!(
                    "dedicated Codex home must not contain {name}"
                )));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    let skills = home.join("skills");
    let metadata = match fs::symlink_metadata(&skills) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    if !metadata.file_type().is_dir() {
        return Err(Error::Invalid(
            "Codex skills must be a regular directory".into(),
        ));
    }
    for entry in fs::read_dir(skills)? {
        let entry = entry?;
        if entry.file_name() != ".system" || !entry.file_type()?.is_dir() {
            return Err(Error::Invalid(
                "dedicated Codex home must not contain custom skills".into(),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_fresh_home_and_generated_system_skills_after_restart() -> Result<()> {
        let home = tempfile::tempdir()?;
        validate_home(home.path())?;
        fs::create_dir_all(home.path().join("skills/.system/openai-docs"))?;
        fs::write(
            home.path()
                .join("skills/.system/.codex-system-skills.marker"),
            "bundled",
        )?;
        validate_home(home.path())?;
        Ok(())
    }

    #[test]
    fn rejects_custom_skills_and_operator_integrations() -> Result<()> {
        for path in [
            "skills/custom",
            "config.toml",
            "AGENTS.md",
            "plugins",
            "hooks.json",
        ] {
            let home = tempfile::tempdir()?;
            fs::create_dir_all(home.path().join(path))?;
            assert!(validate_home(home.path()).is_err());
        }
        Ok(())
    }

    #[test]
    fn rejects_symlinked_skill_roots_and_broken_integration_links() -> Result<()> {
        for path in ["skills", "skills/.system", "config.toml"] {
            let home = tempfile::tempdir()?;
            fs::create_dir_all(home.path().join("skills"))?;
            if path == "skills" {
                fs::remove_dir(home.path().join("skills"))?;
            }
            std::os::unix::fs::symlink("/missing-path", home.path().join(path))?;
            assert!(validate_home(home.path()).is_err());
        }
        Ok(())
    }
}
