use std::path::Path;

use crate::cli::ui;
use crate::core::{CategoryManager, Repository};
use crate::crypto::Crypto;
use crate::error::{ConfectError, Result};
use crate::track::plan::{self, Plan, Scope};
use crate::track::{Metadata, Store};

/// Everything a command needs from an opened repository.
pub struct Ctx {
    pub repo: Repository,
    pub categories: CategoryManager,
    pub metadata: Metadata,
    pub crypto: Crypto,
}

impl Ctx {
    pub fn open(explicit: Option<&Path>) -> Result<Self> {
        Self::from_repo(Repository::open(explicit)?)
    }

    pub fn from_repo(repo: Repository) -> Result<Self> {
        let categories = repo.categories()?;
        let metadata = Metadata::load(repo.path())?;
        let crypto = Crypto::new(
            repo.config().identity_file()?,
            repo.config().recipients_file()?,
        );
        Ok(Self {
            repo,
            categories,
            metadata,
            crypto,
        })
    }

    pub fn store(&self) -> Store<'_> {
        Store::new(self.repo.path(), &self.crypto)
    }

    pub fn plan(&self, scope: &Scope) -> Result<Plan> {
        plan::build(
            self.repo.path(),
            &self.categories,
            &self.metadata,
            &self.store(),
            scope,
        )
    }

    /// Store the state of `scope` in the working tree after a category change.
    ///
    /// Nothing is written — not even the category change — when the result would put a
    /// plaintext secret into the repository.
    pub fn commit_category_change(&mut self, scope: &Scope) -> Result<Plan> {
        self.commit_category_change_for(scope, None)
    }

    /// Like [`Ctx::commit_category_change`], limited to the paths `pattern` covers, so a
    /// pattern edit does not also record unrelated changes of the category.
    pub fn commit_category_change_for(
        &mut self,
        scope: &Scope,
        pattern: Option<&str>,
    ) -> Result<Plan> {
        let mut plan = self.plan(scope)?;
        if let Some(pattern) = pattern {
            let covered = |path: &Path| crate::core::category::pattern_covers(pattern, path);
            plan.changes.retain(|c| covered(&c.path));
            plan.secrets.retain(|s| covered(&s.path));
        }
        ui::print_warnings(&plan.warnings);
        guard_secrets(&plan)?;
        self.categories.save()?;
        let failures = {
            let store = Store::new(self.repo.path(), &self.crypto);
            plan::apply(&plan, &store, &mut self.metadata)?
        };
        self.metadata.save()?;
        report_failures(&failures)?;
        Ok(plan)
    }
}

/// Stop before anything is stored when the plan contains plaintext secrets.
pub fn guard_secrets(plan: &Plan) -> Result<()> {
    if plan.secrets.is_empty() {
        return Ok(());
    }
    ui::print_secrets(&plan.secrets);
    Err(ConfectError::PlaintextSecrets(plan.secrets.len()))
}

pub fn report_failures(failures: &[String]) -> Result<()> {
    if failures.is_empty() {
        return Ok(());
    }
    for failure in failures {
        ui::error_line(failure);
    }
    Err(ConfectError::Other(format!(
        "{} path(s) could not be stored",
        failures.len()
    )))
}
