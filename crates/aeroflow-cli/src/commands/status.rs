use tracing::info;

pub async fn execute() -> anyhow::Result<()> {
    info!("Showing case status");

    println!("\n=== AeroFlow Agent — Case Status ===\n");

    println!("{:<20} {:<15} {:<10} {:<12} {:<10}", "Case", "Stage", "CPU", "Memory", "Est. Rem");
    println!("{:-<20} {:-<15} {:-<10} {:-<12} {:-<10}", "", "", "", "", "");

    // Stub data for P0
    println!("{:<20} {:<15} {:<10} {:<12} {:<10}",
        "wing-v2", "SOLVING", "78% (6c)", "4.2 GB", "00:45:00");
    println!("{:<20} {:<15} {:<10} {:<12} {:<10}",
        "fuse-v1", "MESHING", "45% (4c)", "2.1 GB", "00:08:00");

    println!("\n  {} cases active, {} queued, {} completed", 2, 0, 3);
    println!();

    Ok(())
}
