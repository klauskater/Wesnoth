//! Versioned serialization of consistent store snapshots.
//!
//! VM and presentation caches are deliberately excluded. Contract:
//! `contracts/target/modules/engine/persistence.md`.

use serde::{Deserialize, Serialize};

use super::{protocol::Error, resources::PackageIdentity, store::StoreSnapshot};

const SAVE_VERSION: u32 = 1;

#[derive(Deserialize, Serialize)]
struct Save {
    format_version: u32,
    package: PackageIdentity,
    store: StoreSnapshot,
}

pub fn encode(package: PackageIdentity, store: StoreSnapshot) -> Result<Vec<u8>, Error> {
    serde_json::to_vec_pretty(&Save {
        format_version: SAVE_VERSION,
        package,
        store,
    })
    .map_err(|error| Error::new("invalid_input", format!("cannot encode save: {error}")))
}

pub fn decode(bytes: &[u8], installed: &PackageIdentity) -> Result<StoreSnapshot, Error> {
    let save: Save = serde_json::from_slice(bytes)
        .map_err(|error| Error::new("invalid_input", format!("cannot decode save: {error}")))?;
    if save.format_version != SAVE_VERSION {
        return Err(Error::new(
            "incompatible_version",
            format!(
                "unsupported save version: {} (expected {SAVE_VERSION})",
                save.format_version
            ),
        ));
    }
    if save.package != *installed {
        return Err(Error::new(
            "incompatible_version",
            format!(
                "incompatible package: {} {} protocol {}",
                save.package.package_id,
                save.package.package_version,
                save.package.protocol_version
            ),
        ));
    }
    super::store::Store::from_snapshot(save.store.clone())
        .map_err(|message| Error::new("invalid_input", message))?;
    Ok(save.store)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::engine::store::{Map, Store, World};
    use crate::value::Value;

    fn identity(version: &str) -> PackageIdentity {
        PackageIdentity {
            package_id: "test".into(),
            package_version: version.into(),
            protocol_version: 1,
        }
    }

    #[test]
    fn round_trip_preserves_world_rng_revision_and_rejects_other_package() {
        let mut store = Store::new(
            World {
                map: Map {
                    width: 1,
                    height: 1,
                    cells: vec![Value::Map(BTreeMap::new())],
                },
                entities: BTreeMap::new(),
                data: BTreeMap::new(),
            },
            7,
        )
        .unwrap();
        let mut tx = store.begin(0).unwrap();
        tx.random(1, 10).unwrap();
        store.commit(tx).unwrap();
        let expected = store.snapshot();
        let bytes = encode(identity("1"), expected.clone()).unwrap();
        assert_eq!(decode(&bytes, &identity("1")).unwrap(), expected);
        assert_eq!(
            decode(&bytes, &identity("2")).unwrap_err().code,
            "incompatible_version"
        );
    }
}
