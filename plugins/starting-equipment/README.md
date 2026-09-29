# Starting equipment

Custom Paper plugin providing enchanted equipment to first-time players. The current source is preserved from the existing project, including its local changes.

Build with `gradle build` using Java 21 and the Paper 1.21.11 API declared in `build.gradle.kts`. Gradle is a separate prerequisite; no wrapper or downloaded server library is bundled. The build produces a jar under `build/libs` and does not copy it into a server or restart anything.

The original `build.sh` helper is also preserved. It runs the same Gradle build and copies the resulting jar into this plugin directory; it does not deploy the jar.

Plugin metadata and the Gradle project both declare version 1.0.2. Compatibility with Paper 26.3 has not been established by this consolidation.
