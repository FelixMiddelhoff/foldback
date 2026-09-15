// SPDX-License-Identifier: MIT OR Apache-2.0
using System;
using System.Collections.Generic;
using Foldback;
using UnityEditor;
using UnityEngine;

namespace Foldback.Editor
{
    /// A real in-editor dock for reflective hashing's visibility tooling
    /// (foldback-reflective-hashing.md §4) — the Unity counterpart to
    /// `examples/bevy-editor-demo`'s `bevy_egui` panel. Two independent
    /// views, since Unity has two independent things worth seeing:
    ///
    /// <para><b>Live tab</b>: subscribes to
    /// <see cref="FoldbackReflection.Recorded"/> and lists every
    /// <see cref="FoldbackReflection.HashReflected"/> call made anywhere
    /// in the running game (Play Mode) or Editor tooling, most recent
    /// first — select one to see its field paths and hashes, exactly
    /// what got captured. Closes the same "I tagged this field — can I
    /// actually see it's being hashed" loop the Bevy dock closes, just
    /// event-driven instead of pull-each-frame (Unity has no per-frame
    /// "the dock reads a Resource" ECS convention to lean on the way
    /// Bevy does).</para>
    ///
    /// <para><b>Inspect tab</b>: drag any object in — no Play Mode, no
    /// session, no `HashReflected` call needed — and see
    /// <see cref="FoldbackReflection.ListTracked"/>'s tracked/untracked
    /// split for its actual runtime type. The static-analysis
    /// counterpart to `foldback lint`, but against a real object instead
    /// of source text.</para>
    public sealed class FoldbackReflectionWindow : EditorWindow
    {
        private const int MaxHistory = 100;

        private readonly List<ReflectionPreview> _history = new List<ReflectionPreview>();
        private int _selectedHistoryIndex = -1;
        private Vector2 _historyScroll;
        private Vector2 _fieldsScroll;

        private UnityEngine.Object _inspectTarget;
        private Vector2 _inspectScroll;

        private int _tab;
        private static readonly string[] TabLabels = { "Live", "Inspect" };

        [MenuItem("Window/Foldback/Reflection Inspector")]
        private static void Open()
        {
            GetWindow<FoldbackReflectionWindow>("Foldback Reflection");
        }

        private void OnEnable()
        {
            FoldbackReflection.Recorded += OnRecorded;
        }

        private void OnDisable()
        {
            FoldbackReflection.Recorded -= OnRecorded;
        }

        private void OnRecorded(ReflectionPreview preview)
        {
            _history.Insert(0, preview);
            if (_history.Count > MaxHistory)
            {
                _history.RemoveRange(MaxHistory, _history.Count - MaxHistory);
            }
            // Repaint from whatever thread called HashReflected — Unity's
            // EditorWindow API expects this on the main thread, which is
            // where every documented usage pattern (cookbook recipes 1-2,
            // 8) already calls HashReflected from, same as
            // FoldbackSession's own single-thread-per-Handle assumption.
            Repaint();
        }

        private void OnGUI()
        {
            _tab = GUILayout.Toolbar(_tab, TabLabels);
            EditorGUILayout.Space();
            if (_tab == 0)
            {
                DrawLiveTab();
            }
            else
            {
                DrawInspectTab();
            }
        }

        private void DrawLiveTab()
        {
            if (_history.Count == 0)
            {
                EditorGUILayout.HelpBox(
                    "No HashReflected calls observed yet. Enter Play Mode and run code that " +
                    "calls FoldbackReflection.HashReflected — this tab fills in live.",
                    MessageType.Info);
                return;
            }

            EditorGUILayout.LabelField($"{_history.Count} recorded call(s), most recent first:");
            using (var scroll = new EditorGUILayout.ScrollViewScope(_historyScroll, GUILayout.Height(150)))
            {
                _historyScroll = scroll.scrollPosition;
                for (var i = 0; i < _history.Count; i++)
                {
                    var p = _history[i];
                    var label = $"tick {p.Tick}  entity {p.EntityId}  {p.RootType.Name}  ({p.Fields.Count} field(s))";
                    var wasSelected = i == _selectedHistoryIndex;
                    var isSelected = GUILayout.Toggle(wasSelected, label, EditorStyles.toolbarButton);
                    if (isSelected && !wasSelected)
                    {
                        _selectedHistoryIndex = i;
                    }
                }
            }

            EditorGUILayout.Space();
            if (_selectedHistoryIndex < 0 || _selectedHistoryIndex >= _history.Count)
            {
                EditorGUILayout.HelpBox("Select a call above to see its captured fields.", MessageType.None);
                return;
            }

            var selected = _history[_selectedHistoryIndex];
            EditorGUILayout.LabelField("field", "hash", EditorStyles.boldLabel);
            using var fieldsScroll = new EditorGUILayout.ScrollViewScope(_fieldsScroll);
            _fieldsScroll = fieldsScroll.scrollPosition;
            foreach (var (path, hash) in selected.Fields)
            {
                EditorGUILayout.LabelField(path, hash.ToString("x16"));
            }
        }

        private void DrawInspectTab()
        {
            _inspectTarget = EditorGUILayout.ObjectField(
                "Target", _inspectTarget, typeof(UnityEngine.Object), true);

            if (_inspectTarget == null)
            {
                EditorGUILayout.HelpBox(
                    "Drag a GameObject, component, or asset in to see which of its fields " +
                    "are [FoldbackHash]-tagged — no Play Mode or session required.",
                    MessageType.Info);
                return;
            }

            var type = _inspectTarget.GetType();
            var (tracked, untracked) = FoldbackReflection.ListTracked(type);

            EditorGUILayout.LabelField($"Type: {type.FullName}", EditorStyles.boldLabel);
            using var scroll = new EditorGUILayout.ScrollViewScope(_inspectScroll);
            _inspectScroll = scroll.scrollPosition;

            EditorGUILayout.LabelField($"Tracked ({tracked.Count}) — [FoldbackHash]", EditorStyles.boldLabel);
            if (tracked.Count == 0)
            {
                EditorGUILayout.LabelField("(none)");
            }
            foreach (var name in tracked)
            {
                EditorGUILayout.LabelField("  " + name);
            }

            EditorGUILayout.Space();
            EditorGUILayout.LabelField($"Untracked ({untracked.Count})", EditorStyles.boldLabel);
            if (untracked.Count == 0)
            {
                EditorGUILayout.LabelField("(none)");
            }
            foreach (var name in untracked)
            {
                EditorGUILayout.LabelField("  " + name);
            }
        }
    }
}
