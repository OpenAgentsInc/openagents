//! Tests for the bootloader module.

use crate::app::bootloader::{
    BootEvent, BootStage, BootloaderConfig, BootloaderModule, StageDetails,
};
use tokio::sync::mpsc;

#[tokio::test]
async fn test_bootloader_full_sequence() {
    let bootloader = BootloaderModule::new();
    let manifest = bootloader.run().await.unwrap();

    // Should have discovered hardware
    assert!(manifest.hardware.cpu_cores > 0, "CPU cores should be > 0");
    assert!(manifest.hardware.ram_gb > 0.0, "RAM should be > 0");

    // Boot duration should be reasonable
    assert!(
        manifest.boot_duration.as_secs() < 30,
        "Boot should complete in < 30s"
    );
}

#[tokio::test]
async fn test_bootloader_minimal_config() {
    let config = BootloaderConfig::minimal();
    let bootloader = BootloaderModule::with_config(config);
    let manifest = bootloader.run().await.unwrap();

    // Hardware should still be populated
    assert!(manifest.hardware.cpu_cores > 0);

    // Compute should be empty (skipped)
    assert!(
        manifest.compute.backends.is_empty(),
        "Compute backends should be empty when skipped"
    );

    // Network should be offline (skipped)
    assert!(
        !manifest.network.has_internet,
        "Network should show offline when skipped"
    );
}

#[tokio::test]
async fn test_bootloader_offline_config() {
    let config = BootloaderConfig::offline();
    let bootloader = BootloaderModule::with_config(config);
    let manifest = bootloader.run().await.unwrap();

    // Network should be offline
    assert!(
        !manifest.network.has_internet,
        "Network should be offline when configured"
    );
}

#[tokio::test]
async fn test_bootloader_event_emission() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let config = BootloaderConfig::minimal();
    let bootloader = BootloaderModule::with_config(config).with_events(tx);

    // Run bootloader in background
    let handle = tokio::spawn(async move { bootloader.run().await });

    // Collect events
    let mut events = Vec::new();
    while let Some(event) = rx.recv().await {
        let is_completed = matches!(&event, BootEvent::BootCompleted { .. });
        events.push(event);
        if is_completed {
            break;
        }
    }

    // Should have received BootStarted
    assert!(
        matches!(events.first(), Some(BootEvent::BootStarted { .. })),
        "First event should be BootStarted"
    );

    // Should have received BootCompleted
    assert!(
        matches!(events.last(), Some(BootEvent::BootCompleted { .. })),
        "Last event should be BootCompleted"
    );

    // Should have hardware stage completed
    let has_hardware_completed = events.iter().any(|e| {
        matches!(
            e,
            BootEvent::StageCompleted {
                stage: BootStage::Hardware,
                ..
            }
        )
    });
    assert!(
        has_hardware_completed,
        "Should have Hardware stage completed"
    );

    // Wait for the spawned task
    let _ = handle.await;
}

#[tokio::test]
async fn test_bootloader_stage_details() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let config = BootloaderConfig {
        skip_compute: true,
        skip_network: true,
        skip_identity: true,
        skip_workspace: true,
        skip_summary: true,
        ..Default::default()
    };
    let bootloader = BootloaderModule::with_config(config).with_events(tx);

    tokio::spawn(async move { bootloader.run().await });

    // Find hardware stage completion
    while let Some(event) = rx.recv().await {
        if let BootEvent::StageCompleted {
            stage: BootStage::Hardware,
            details: StageDetails::Hardware(hw),
            ..
        } = event
        {
            assert!(hw.cpu_cores > 0, "Hardware details should have CPU cores");
            assert!(hw.ram_gb > 0.0, "Hardware details should have RAM");
            break;
        }
        if matches!(event, BootEvent::BootCompleted { .. }) {
            break;
        }
    }
}

#[test]
fn test_bootloader_config_defaults() {
    let config = BootloaderConfig::default();
    assert!(!config.skip_hardware);
    assert!(!config.skip_compute);
    assert!(!config.skip_network);
    assert!(!config.skip_identity);
    assert!(!config.skip_workspace);
    assert!(!config.skip_summary);
}

#[test]
fn test_bootloader_config_minimal() {
    let config = BootloaderConfig::minimal();
    assert!(!config.skip_hardware);
    assert!(config.skip_compute);
    assert!(config.skip_network);
}

#[test]
fn test_bootloader_config_offline() {
    let config = BootloaderConfig::offline();
    assert!(!config.skip_hardware);
    assert!(!config.skip_compute);
    assert!(config.skip_network);
}

#[test]
fn test_boot_stage_names() {
    assert_eq!(BootStage::Hardware.name(), "Hardware");
    assert_eq!(BootStage::Compute.name(), "Compute");
    assert_eq!(BootStage::Network.name(), "Network");
    assert_eq!(BootStage::Identity.name(), "Identity");
    assert_eq!(BootStage::Workspace.name(), "Workspace");
    assert_eq!(BootStage::Summary.name(), "Summary");
}

#[test]
fn test_boot_stage_descriptions() {
    assert!(!BootStage::Hardware.description().is_empty());
    assert!(!BootStage::Compute.description().is_empty());
    assert!(!BootStage::Network.description().is_empty());
    assert!(!BootStage::Identity.description().is_empty());
    assert!(!BootStage::Workspace.description().is_empty());
    assert!(!BootStage::Summary.description().is_empty());
}
