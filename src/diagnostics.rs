use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ScanDiagnostics {
    pub roots: Vec<PathBuf>,
    pub visited: usize,
    pub cancelled: Option<bool>,
    pub errors: Vec<String>,
    pub skipped: Vec<String>,
    pub error_count: usize,
    pub skipped_count: usize,
    pub filtered_count: usize,
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
        let state = match self.cancelled {
            Some(true) => "Interrompu",
            Some(false) => "Terminé",
            None => "État final non enregistré",
        };
        let roots = self
            .roots
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>()
            .join("\n");
        format!("RustyShelf — rapport du dernier scan\nÉtat : {state}\n{} éléments parcourus\n{} ancien(s) candidat(s) non reconnu(s) masqué(s) (restaurables dans les options)\n\nDISQUES ET DOSSIERS CHOISIS\n{roots}\n\n{} erreur(s), {} chemin(s) non analysé(s)\nDétail limité aux 50 premiers messages par type.\n\nERREURS\n{}\n\nCHEMINS NON ANALYSÉS\n{}\n", self.visited, self.filtered_count, self.error_count, self.skipped_count, self.errors.join("\n"), self.skipped.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn old_reports_still_load_and_new_reports_preserve_scan_scope_and_outcome() {
        let old: ScanDiagnostics = serde_json::from_str(
            r#"{"errors":["Erreur"],"skipped":[],"error_count":1,"skipped_count":0}"#,
        )
        .unwrap();
        assert_eq!(old.error_count, 1);
        assert_eq!(old.filtered_count, 0);
        assert!(old.cancelled.is_none());
        let report = ScanDiagnostics {
            roots: vec!["/roms/Été".into()],
            visited: 42,
            filtered_count: 3,
            cancelled: Some(true),
            ..Default::default()
        };
        let loaded: ScanDiagnostics =
            serde_json::from_str(&serde_json::to_string(&report).unwrap()).unwrap();
        assert_eq!(loaded.roots, report.roots);
        assert_eq!(loaded.visited, 42);
        assert_eq!(loaded.filtered_count, 3);
        assert!(loaded.report().contains("3 ancien(s) candidat(s)"));
        assert_eq!(loaded.cancelled, Some(true));
        assert!(loaded.report().contains("Interrompu"));
        assert!(loaded.report().contains("/roms/Été"));
    }
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
