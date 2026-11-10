use std::{
    fs::{self, File},
    io::{BufWriter, Write},
    path::PathBuf,
};

use anyhow::{bail, Context};
use clap::Parser;
use rbx_reflection::ReflectionDatabase;
use rmp_serde::Serializer;
use serde::Serialize;
use tempfile::tempdir;

use rbx_reflection_gen::{apply_dump, patches::PatchSources};

use crate::defaults::apply_defaults;

use super::{defaults_place::DefaultsPlaceSubcommand, dump::DumpSubcommand};

/// Generate a reflection database from the system's Roblox Studio installation
/// and write it to disk.
#[derive(Debug, Parser)]
pub struct GenerateSubcommand {
    #[clap(long)]
    pub patches: Option<PathBuf>,
    /// Where to output the reflection database. The output format is inferred
    /// from the file path and supports JSON (.json) and MessagePack (.msgpack).
    pub output: Vec<PathBuf>,
    /// Whether to pretty-print the JSON output. This has no effect on MessagePack.
    #[clap(long)]
    pub no_pretty: bool,
    /// Whether to serialize MessagePack in a human readable format. This has no effect on JSON.
    #[clap(long)]
    pub human_readable: bool,
}

impl GenerateSubcommand {
    pub fn run(&self) -> anyhow::Result<()> {
        let temp_dir = tempdir()?;
        let api_dump_path = temp_dir.path().join("api-dump.json");
        let defaults_place_path = temp_dir.path().join("defaults-place.rbxlx");

        DumpSubcommand {
            output: api_dump_path.clone(),
        }
        .run()?;

        let contents = fs::read_to_string(&api_dump_path).context("Could not read API dump")?;
        let dump = serde_json::from_str(&contents).context("Invalid API dump")?;

        let studio_info = DefaultsPlaceSubcommand {
            api_dump: api_dump_path,
            output: defaults_place_path.clone(),
        }
        .run()?;

        let mut database = ReflectionDatabase::new();

        apply_dump(&mut database, &dump)?;

        let patch_sources;
        let patches;
        if let Some(patches_path) = &self.patches {
            patch_sources = PatchSources::load(patches_path)?;
            patches = patch_sources.parse()?;

            patches.apply_pre_default(&mut database)?;
            apply_defaults(&mut database, &defaults_place_path)?;
            patches.apply_post_default(&mut database)?;
            patches.validate_migrations(&database)?;
        } else {
            apply_defaults(&mut database, &defaults_place_path)?;
        }

        database.version = studio_info.version;

        for path in &self.output {
            let extension = path.extension().unwrap_or_default().to_str();

            let mut file = BufWriter::new(File::create(path)?);

            match extension {
                Some("json") => {
                    let result = if self.no_pretty {
                        serde_json::to_writer(&mut file, &database)
                    } else {
                        serde_json::to_writer_pretty(&mut file, &database)
                    };

                    result.context("Could not serialize reflection database as JSON")?;
                }
                Some("msgpack") => {
                    let buf = if self.human_readable {
                        let mut slice = Vec::with_capacity(128);
                        let mut serializer = Serializer::new(&mut slice)
                            .with_human_readable()
                            .with_struct_map();

                        database.serialize(&mut serializer).context(
                            "Could not serialize reflection database as human readable MessagePack",
                        )?;

                        slice
                    } else {
                        rmp_serde::to_vec(&database)
                            .context("Could not serialize reflection database as MessagePack")?
                    };

                    file.write_all(&buf)?;
                }
                _ => bail!(
                    "Unknown output for path {} -- \
                    Supported formats are JSON (.json) and MessagePack (.msgpack)",
                    path.display()
                ),
            }

            file.flush()?;
        }

        Ok(())
    }
}

