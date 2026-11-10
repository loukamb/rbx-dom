//! Reflection database generation shared by `rbx_reflector` and Isu.
//!
//! This crate exists in Isu's fork of rbx-dom so the API-dump → database step can be
//! used as a library, without the Studio automation and network dependencies of the
//! `rbx_reflector` binary.

pub mod api_dump;
mod apply;
pub mod patches;

pub use apply::apply_dump;

use rbx_reflection::ReflectionDatabase;

use crate::{api_dump::Dump, patches::Patches};

/// Build a database from an API dump and parsed patches, without class defaults (which
/// require saving a place in Studio). Patches are applied before and after the point
/// where defaults would be applied, exactly as `rbx_reflector generate` does.
pub fn database_from_dump<'db>(
    dump: &'db Dump,
    patches: &'db Patches<'db>,
) -> anyhow::Result<ReflectionDatabase<'db>> {
    let mut database = ReflectionDatabase::new();
    apply_dump(&mut database, dump)?;
    patches.apply_pre_default(&mut database)?;
    patches.apply_post_default(&mut database)?;
    patches.validate_migrations(&database)?;
    Ok(database)
}
