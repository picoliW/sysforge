Output docs/media/demo.gif

# --- terminal look ---
Set Theme "Catppuccin Mocha"
Set FontSize 15
Set Width 1200
Set Height 700
Set Padding 20
Set PlaybackSpeed 1.0

# --- launch ---
Hide
Type "cargo run --release" Enter
Sleep 3s
Show

# --- 1. Overview: let the live gauges breathe ---
Sleep 3s

# --- 2. Docker (view 2): select a container, open its logs ---
Type "2"
Sleep 1800ms
Down
Sleep 700ms
Type "l"
Sleep 2200ms
Escape
Sleep 700ms
Type "a"
Sleep 1000ms
Escape
Sleep 700ms

# --- 3. Processes (view 3): the biggest table, sorted live ---
Type "3"
Sleep 2s
Down Down Down
Sleep 1200ms

# --- 4. Git (view 4): the working repository ---
Type "4"
Sleep 2s

# --- 5. Network (view 5): throughput sparklines ---
Type "5"
Sleep 2s

# --- 6. Disk (view 6): usage and I/O ---
Type "6"
Sleep 2s

# --- 7. systemd (view 7): services, failed first ---
Type "7"
Sleep 2s

# --- 8. Kubernetes (view 8): the finale — live pods, colored by status ---
Type "8"
Sleep 2500ms
Down Down
Sleep 1500ms

# --- the help overlay: shows the whole keymap ---
Type "?"
Sleep 3s
Escape
Sleep 500ms

# --- back to overview, quit cleanly ---
Type "1"
Sleep 1500ms
Type "q"
Sleep 1s
