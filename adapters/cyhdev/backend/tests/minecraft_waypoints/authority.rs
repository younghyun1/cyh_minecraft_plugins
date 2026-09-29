//! A blocked waypoint transaction retains its authority lease through commit.

use crate::{
    marker,
    support::{
        database::{DatabaseTestFuture, TestDatabase, TestResult, require, run_database_test},
        fixtures::{account_test_context, seed_account},
    },
};
use diesel::QueryDsl;
use diesel_async::{AsyncConnection, RunQueryDsl};
use rust_be_template::{
    features::{
        accounts::domain::role::RoleType,
        minecraft::{
            error::MapError, repository::waypoints::WaypointRepository,
            service::waypoints::WaypointService,
        },
    },
    schema::minecraft_map_world,
};
use std::{sync::Arc, time::Duration};

#[tokio::test]
#[ignore = "requires explicit TEST_DATABASE_URL and PostgreSQL 18"]
async fn role_demotion_waits_for_in_progress_waypoint_commit() -> TestResult {
    run_database_test(lease_case).await
}

fn lease_case(database: &TestDatabase) -> DatabaseTestFuture<'_> {
    Box::pin(async move {
        let context = account_test_context(database)?;
        let actor = seed_account(&context, "WaypointLeaseOwner").await?;
        context
            .accounts
            .assign_role(actor.user_id, RoleType::Younghyun)
            .await?;
        let service = Arc::new(WaypointService::new(
            WaypointRepository::new(context.pool.clone()),
            Arc::clone(&context.accounts),
        ));
        service
            .save(
                actor.user_id,
                None,
                marker("minecraft:overworld", "Initial"),
            )
            .await?;
        let pool = context.pool.clone();
        let (locked_tx, locked_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = tokio::sync::oneshot::channel();
        let lock_task = tokio::spawn(async move {
            let mut connection = pool.get().await.map_err(MapError::Pool)?;
            connection
                .transaction::<(), MapError, _>(async move |connection| {
                    minecraft_map_world::table
                        .find("minecraft:overworld")
                        .select(minecraft_map_world::minecraft_map_world_id)
                        .for_update()
                        .first::<String>(connection)
                        .await?;
                    let _ = locked_tx.send(());
                    let _ = release_rx.await;
                    Ok(())
                })
                .await
        });
        locked_rx.await?;
        let writer = Arc::clone(&service);
        let mut write_task = tokio::spawn(async move {
            writer
                .save(
                    actor.user_id,
                    None,
                    marker("minecraft:overworld", "Pending"),
                )
                .await
        });
        require(
            tokio::time::timeout(Duration::from_millis(100), &mut write_task)
                .await
                .is_err(),
            "write ignored the world row lock",
        )?;
        let accounts = Arc::clone(&context.accounts);
        let mut demote_task =
            tokio::spawn(async move { accounts.assign_role(actor.user_id, RoleType::User).await });
        require(
            tokio::time::timeout(Duration::from_millis(100), &mut demote_task)
                .await
                .is_err(),
            "role demotion crossed an active authority lease",
        )?;
        let _ = release_tx.send(());
        lock_task.await??;
        write_task.await??;
        demote_task.await??;
        require(
            matches!(
                service
                    .save(actor.user_id, None, marker("minecraft:overworld", "Denied"))
                    .await,
                Err(MapError::Authority(_))
            ),
            "demoted user retained write authority",
        )?;
        require(
            service.list("minecraft:overworld").await?.len() == 2,
            "pending write was not committed exactly once",
        )
    })
}
