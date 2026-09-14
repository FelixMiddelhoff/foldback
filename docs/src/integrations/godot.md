# Godot

## Status

**Not started.** Planned for Phase 4, after the Unity binding. Will be a GDExtension wrapper exposing the core through a GDScript-native `FoldbackSession` class rather than raw FFI calls, matching how other GDExtension addons present themselves.

## In the meantime

The target API shape is sketched in [Cookbook recipe 9](../cookbook/README.md#9-godot-integration) — subject to change before it's actually implemented.

## Who this will be for

Godot projects (GDScript or C#) using a lockstep or rollback netcode approach.
