use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ScanDiagnostics {
    pub errors: Vec<String>,
    pub skipped: Vec<String>,
    pub error_count: usize,
    pub skipped_count: usize,
}

impl ScanDiagnostics {
    pub fn error(&mut self, message: String) {
        self.error_count += 1;
        if self.errors.len() < 50 {
            self.errors.push(message);
        }
    }

    pub fn skip(&mut self, path: &Path, reason: &str) {
        self.skipped_count += 1;
        if self.skipped.len() < 50 {
            self.skipped.push(format!("{} : {reason}", path.display()));
        }
    }

    pub fn report(&self) -> String {
        format!("RustyShelf — rapport du dernier scan\n{} erreur(s), {} chemin(s) non analysé(s)\nDétail limité aux 50 premiers messages par type.\n\nERREURS\n{}\n\nCHEMINS NON ANALYSÉS\n{}\n", self.error_count, self.skipped_count, self.errors.join("\n"), self.skipped.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn keeps_accurate_counts_without_growing_unbounded() {
        let mut diagnostics = ScanDiagnostics::default();
        for _ in 0..200 {
            diagnostics.skip(Path::new("/protected"), "Accès refusé");
        }
        diagnostics.error("Base en lecture seule".into());
        assert_eq!(diagnostics.skipped_count, 200);
        assert_eq!(diagnostics.skipped.len(), 50);
        assert_eq!(diagnostics.error_count, 1);
        assert!(diagnostics.report().contains("Base en lecture seule"));
    }
}
