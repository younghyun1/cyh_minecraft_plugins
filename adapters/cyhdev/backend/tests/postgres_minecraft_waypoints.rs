//! Public marker persistence, current authority, hard capacity, and cross-world moves.

mod support;

use diesel::{ExpressionMethods, QueryDsl};
use diesel_async::RunQueryDsl;
use rust_be_template::{
    features::{
        accounts::domain::role::RoleType,
        minecraft::{
            domain::waypoint::WaypointInput, error::MapError,
            repository::waypoints::WaypointRepository, service::waypoints::WaypointService,
        },
    },
    schema::minecraft_waypoint,
};
use std::sync::Arc;
use support::{
    database::{DatabaseTestFuture, TestDatabase, TestResult, require, run_database_test},
    fixtures::{account_test_context, seed_account},
};

fn marker(world: &str, name: &str) -> WaypointInput {
    WaypointInput {
        world: world.into(),
        name: name.into(),
        description: "Public meeting place".into(),
        x: -30_000_000,
        y: -2032,
        z: 30_000_000,
    }
}

#[tokio::test]
#[ignore = "requires explicit TEST_DATABASE_URL and PostgreSQL 18"]
async fn public_markers_require_current_admin_for_every_mutation() -> TestResult {
    run_database_test(authority_case).await
}

fn authority_case(database: &TestDatabase) -> DatabaseTestFuture<'_> {
    Box::pin(async move {
        let context = account_test_context(database)?;
        let actor = seed_account(&context, "WaypointOwner").await?;
        let service = WaypointService::new(
            WaypointRepository::new(context.pool.clone()),
            Arc::clone(&context.accounts),
        );
        let input = marker("minecraft:overworld", "家");
        require(
            matches!(
                service.save(actor.user_id, None, input.clone()).await,
                Err(MapError::Authority(_))
            ),
            "ordinary user created a waypoint",
        )?;
        context
            .accounts
            .assign_role(actor.user_id, RoleType::Younghyun)
            .await?;
        let created = service.save(actor.user_id, None, input.clone()).await?;
        require(
            created.id.get_version_num() == 7,
            "waypoint identifier was not generated as UUIDv7",
        )?;
        let public = service.list("minecraft:overworld").await?;
        require(
            public.len() == 1 && public[0].input.name == "家",
            "public listing did not preserve marker",
        )?;
        let moved = service
            .save(
                actor.user_id,
                Some(created.id),
                marker("minecraft:the_nether", "Portal"),
            )
            .await?;
        require(
            moved.id == created.id && service.list("minecraft:overworld").await?.is_empty(),
            "move duplicated or retained marker",
        )?;
        context
            .accounts
            .assign_role(actor.user_id, RoleType::User)
            .await?;
        require(
            matches!(
                service.save(actor.user_id, Some(created.id), input).await,
                Err(MapError::Authority(_))
            ),
            "demoted user updated marker",
        )?;
        require(
            matches!(
                service.delete(actor.user_id, created.id).await,
                Err(MapError::Authority(_))
            ),
            "demoted user deleted marker",
        )?;
        context
            .accounts
            .assign_role(actor.user_id, RoleType::Younghyun)
            .await?;
        service.delete(actor.user_id, created.id).await?;
        require(
            service.list("minecraft:the_nether").await?.is_empty(),
            "deleted marker remained public",
        )?;
        require(
            matches!(
                service.delete(actor.user_id, created.id).await,
                Err(MapError::NotFound)
            ),
            "missing marker deletion did not return absence",
        )
    })
}

#[tokio::test]
#[ignore = "requires explicit TEST_DATABASE_URL and PostgreSQL 18"]
async fn concurrent_creates_and_moves_preserve_the_database_capacity() -> TestResult {
    run_database_test(capacity_case).await
}

fn capacity_case(database: &TestDatabase) -> DatabaseTestFuture<'_> {
    Box::pin(async move {
        let repository = WaypointRepository::new(database.pool()?);
        for index in 0..255 {
            repository
                .save(
                    None,
                    marker("minecraft:overworld", &format!("Marker {index}")),
                )
                .await?;
        }
        let (first, second) = tokio::join!(
            repository.save(None, marker("minecraft:overworld", "First")),
            repository.save(None, marker("minecraft:overworld", "Second"))
        );
        require(
            first.is_ok() != second.is_ok(),
            "concurrent creates did not consume exactly one remaining slot",
        )?;
        require(
            matches!(first, Err(MapError::Full)) || matches!(second, Err(MapError::Full)),
            "capacity failure was not classified",
        )?;
        let outsider = repository
            .save(None, marker("minecraft:the_end", "End"))
            .await?;
        require(
            matches!(
                repository
                    .save(Some(outsider.id), marker("minecraft:overworld", "Move"))
                    .await,
                Err(MapError::Full)
            ),
            "move exceeded destination capacity",
        )?;
        require(
            repository.list("minecraft:the_end").await?.len() == 1,
            "failed move lost original marker",
        )?;
        let full = repository.list("minecraft:overworld").await?;
        require(full.len() == 256, "world capacity was exceeded")?;
        let Some(first) = full.as_slice().first() else {
            return require(false, "full world was empty");
        };
        repository.delete(first.id).await?;
        repository
            .save(Some(outsider.id), marker("minecraft:overworld", "Move"))
            .await?;
        require(
            repository.list("minecraft:overworld").await?.len() == 256,
            "vacated slot was not reused",
        )?;
        let pool = database.pool()?;
        let mut connection = pool.get().await?;
        let invalid = diesel::update(minecraft_waypoint::table.find(outsider.id))
            .set(minecraft_waypoint::minecraft_waypoint_slot.eq(256i16))
            .execute(&mut connection)
            .await;
        require(
            matches!(
                invalid,
                Err(diesel::result::Error::DatabaseError(
                    diesel::result::DatabaseErrorKind::CheckViolation,
                    _
                ))
            ),
            "database accepted an out-of-range slot",
        )?;
        let invalid = diesel::update(minecraft_waypoint::table.find(outsider.id))
            .set(minecraft_waypoint::minecraft_waypoint_name.eq("家".repeat(81)))
            .execute(&mut connection)
            .await;
        require(
            matches!(
                invalid,
                Err(diesel::result::Error::DatabaseError(
                    diesel::result::DatabaseErrorKind::CheckViolation,
                    _
                ))
            ),
            "database accepted an oversized Unicode name",
        )
    })
}

#[tokio::test]
#[ignore = "requires explicit TEST_DATABASE_URL and PostgreSQL 18"]
async fn waypoint_rollback_preserves_annotations_until_explicitly_removed() -> TestResult {
    run_database_test(rollback_case).await
}

fn rollback_case(database: &TestDatabase) -> DatabaseTestFuture<'_> {
    Box::pin(async move {
        let repository = WaypointRepository::new(database.pool()?);
        let created = repository
            .save(None, marker("minecraft:overworld", "Rollback guard"))
            .await?;
        let url = database.database_url().to_owned();
        tokio::task::spawn_blocking(move || -> TestResult {
            use diesel::{Connection, pg::PgConnection};
            use support::migrations::{latest_down_is_refused, rewind_to_migration};
            let mut connection = PgConnection::establish(&url)?;
            rewind_to_migration(&mut connection, "20260928000001")?;
            latest_down_is_refused(&mut connection)
        })
        .await??;
        repository.delete(created.id).await?;
        let url = database.database_url().to_owned();
        tokio::task::spawn_blocking(move || -> TestResult {
            use diesel::{Connection, pg::PgConnection};
            use diesel_migrations::MigrationHarness;
            use rust_be_template::init::db_migrations::MIGRATIONS;
            let mut connection = PgConnection::establish(&url)?;
            connection.revert_last_migration(MIGRATIONS)?;
            connection.run_pending_migrations(MIGRATIONS)?;
            Ok(())
        })
        .await??;
        require(
            repository.list("minecraft:overworld").await?.is_empty(),
            "rollback/reapply did not restore empty marker storage",
        )
    })
}

mod minecraft_waypoints;
