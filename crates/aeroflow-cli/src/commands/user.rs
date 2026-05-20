use aeroflow_core::{CreateUserRequest, UpdateUserRequest, UserRole};
use aeroflow_skills::UserManager;

async fn get_manager() -> anyhow::Result<UserManager> {
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://aeroflow@localhost:5432/aeroflow".to_string());
    let pool = sqlx::postgres::PgPool::connect(&database_url).await?;
    Ok(UserManager::new(pool))
}

pub async fn execute(action: &crate::UserAction) -> anyhow::Result<()> {
    match action {
        crate::UserAction::Create => {
            println!("\n=== AeroFlow — Create User ===\n");

            let name: String = dialoguer::Input::new()
                .with_prompt("Full name")
                .interact_text()?;

            let email: String = dialoguer::Input::new()
                .with_prompt("Email")
                .interact_text()?;

            let password: String = dialoguer::Input::new()
                .with_prompt("Password")
                .interact_text()?;

            let role_idx = dialoguer::Select::new()
                .with_prompt("Role")
                .items(&["engineer", "admin", "viewer"])
                .default(0)
                .interact()?;

            let role = match role_idx {
                0 => UserRole::Engineer,
                1 => UserRole::Admin,
                _ => UserRole::Viewer,
            };

            let mgr = get_manager().await?;
            let user = mgr.create_user(&CreateUserRequest {
                name, email, password, role,
            }).await?;

            println!("\n✓ User created: {} ({}) — role={}", user.name, user.email, user.role.label());
        }

        crate::UserAction::List => {
            let mgr = get_manager().await?;
            let users = mgr.list_users().await?;

            println!("\n=== AeroFlow — User List ===\n");
            println!("{:<40} {:<25} {:<12} {:<8}", "Name", "Email", "Role", "Active");
            println!("{:-<40} {:-<25} {:-<12} {:-<8}", "", "", "", "");

            for u in &users {
                let active = if u.active { "✓" } else { "✗" };
                println!("{:<40} {:<25} {:<12} {:<8}",
                    u.name, u.email, u.role.label(), active);
            }
            println!("\n  {} user(s)", users.len());
        }

        crate::UserAction::Show { email } => {
            let mgr = get_manager().await?;
            let user = mgr.get_user_by_email(email).await?;

            match user {
                Some(u) => {
                    println!("\n=== User: {} ===\n", u.name);
                    println!("  ID:        {}", u.id);
                    println!("  Email:     {}", u.email);
                    println!("  Role:      {}", u.role.label());
                    println!("  Active:    {}", if u.active { "yes" } else { "no" });
                    println!("  Last login: {}", u.last_login.map(|t| t.to_string()).unwrap_or_else(|| "never".into()));
                    println!("  Max cases: {}", u.quota_max_concurrent);
                    println!("  Max cores: {}", u.quota_max_cores);
                    println!("  Max mem:   {} GB", u.quota_max_memory_gb);
                    println!("  Created:   {}", u.created_at);
                }
                None => println!("User not found: {}", email),
            }
        }

        crate::UserAction::Update { email } => {
            let mgr = get_manager().await?;
            let user = mgr.get_user_by_email(email).await?;
            let user = match user {
                Some(u) => u,
                None => {
                    println!("User not found: {}", email);
                    return Ok(());
                }
            };

            println!("\n=== Update User: {} ===\n", user.name);
            println!("(Leave blank to keep current value)\n");

            let name: String = dialoguer::Input::new()
                .with_prompt("Name")
                .default(user.name.clone())
                .interact_text()?;

            let new_email: String = dialoguer::Input::new()
                .with_prompt("Email")
                .default(user.email.clone())
                .interact_text()?;

            let role_idx = dialoguer::Select::new()
                .with_prompt("Role")
                .items(&["engineer", "admin", "viewer"])
                .default(match user.role {
                    UserRole::Engineer => 0,
                    UserRole::Admin => 1,
                    UserRole::Viewer => 2,
                })
                .interact()?;

            let role = match role_idx {
                0 => Some(UserRole::Engineer),
                1 => Some(UserRole::Admin),
                _ => Some(UserRole::Viewer),
            };

            let updated = mgr.update_user(user.id, &UpdateUserRequest {
                name: Some(name),
                email: Some(new_email),
                password: None,
                role,
                active: Some(true),
                quota_max_concurrent: None,
                quota_max_cores: None,
                quota_max_memory_gb: None,
                preferences: None,
            }).await?;

            println!("\n✓ User updated: {} ({})", updated.name, updated.email);
        }

        crate::UserAction::Delete { email } => {
            let mgr = get_manager().await?;
            let user = mgr.get_user_by_email(email).await?;
            let user = match user {
                Some(u) => u,
                None => {
                    println!("User not found: {}", email);
                    return Ok(());
                }
            };

            println!("\n=== Delete User: {} ===\n", user.name);
            let confirm = dialoguer::Confirm::new()
                .with_prompt("Are you sure?")
                .default(false)
                .interact()?;

            if confirm {
                mgr.delete_user(user.id).await?;
                println!("✓ User deleted: {} ({})", user.name, user.email);
            } else {
                println!("Cancelled.");
            }
        }

        crate::UserAction::Login { email } => {
            let password: String = dialoguer::Input::new()
                .with_prompt("Password")
                .interact_text()?;

            let mgr = get_manager().await?;
            let user = mgr.authenticate(email, &password).await?;

            match user {
                Some(u) => {
                    println!("\n✓ Authenticated: {} ({})", u.name, u.email);
                    let session = mgr.create_session(u.id, 24).await?;
                    println!("  Session token: {}", session.token);
                    println!("  Expires: {}", session.expires_at);
                }
                None => {
                    println!("\n✗ Authentication failed.");
                }
            }
        }
    }

    Ok(())
}
