// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 Busbar Inc and contributors

//! Actual dropped image identity; ranking behavior is exercised in ordinary door witnesses.

#[test]
fn the_actual_dropped_image_is_distinct_from_the_linked_image() {
    let path = std::path::PathBuf::from(
        std::env::var_os("BUSBAR_PLUGIN_CDYLIB")
            .expect("build the actual ranking cdylib freshly and set BUSBAR_PLUGIN_CDYLIB"),
    );
    assert!(
        path.is_absolute() && path.is_file(),
        "an absolute existing actual image is required"
    );
    let (dropped_door, dropped_ops) =
        busbar_plugin_loader::dispatch::load::door_addresses_of_library(&path)
            .expect("the actual dropped image loads")
            .expect("the image exports its actual door");
    let linked = busbar_hook_ranking::linked::door();
    assert_ne!(
        dropped_door,
        std::ptr::from_ref(linked) as usize,
        "different physical doors"
    );
    assert_ne!(
        dropped_ops,
        linked.parts().ops as usize,
        "different physical operation tables"
    );
}
