# The optional desktop: a tiling session whose default window is Coder.
#
# A host that leaves `coderos.desktop.enable` off gets nothing from this file.
# Every setting below sits under one `lib.mkIf`, so the closure of a host with
# the option off is the closure it had before this file existed.
#
# `coderos.desktop.compositor` says which compositor the tty1 login starts:
# Hyprland, which this file writes a configuration for, or the Coder
# compositor, which reads its start list from a grant instead.
# `coderos.desktop.trialTty` runs the other one on a second TTY, so one host
# runs both and the console keys reach either. Which compositor tty1 starts
# says what the second TTY is for: beside a Hyprland default it is where the
# Coder compositor earns the switch, and beside a Coder default it is the way
# back to the session you already know. A host at `hyprland` with no trial
# TTY builds no compositor package, writes no grant, and gets the system it
# got before the options existed.
#
# The Coder compositor is `crates/coder-compositor`, built by
# `os/pkgs/coder-compositor.nix`, and its session is
# `os/bin/coder-compositor-session`. Both are option defaults, so a host that
# never runs the compositor never builds it.
#
# Every `bind` and `windowrule` line below is a row of `crates/coder-binds`,
# which the Coder compositor reads directly, and
# `crates/coder-binds/tests/binds.rs` fails when a line and its row disagree.
# A module that lives in a host's own flake adds its launchers and window
# rules through `extraBinds` and `extraWindowRules`, which reach both
# compositors, rather than through a row here.
{ config, lib, pkgs, ... }:

let
  cfg = config.coderos.desktop;

  # The OpenAgents white ladder: four intensities of white on near-black,
  # the same values the website (`crates/openagents-web/src/palette.rs`)
  # and the desktop theme use. It replaced the amber ladder on 2026-09-30.
  # The copies here are copies because a Nix file cannot read a Rust
  # constant, and they match the console palette `default.nix` sets.
  # Paper Mono, the one typeface every OpenAgents surface uses, from the
  # files committed under `crates/paper-mono/fonts/`.
  paperMono = pkgs.callPackage ../../pkgs/paper-mono.nix { };

  white = "ffffff";
  white25 = "4a4a4a";
  white50 = "8a8a8a";
  white75 = "c8c8c8";
  nearBlackTinted = "1a1a1a";
  nearBlack = "0a0a0a";

  # foot has its own 16-colour palette; without it a program that asks for
  # green (the bash prompt is bold green) draws green on this white desktop.
  # Map every ANSI slot to a rung of the white ladder, the same mapping
  # console.colors makes for the VGA console, so foot is white whichever slot
  # a program reaches for.
  footPalette = ''
    regular0=${nearBlack}
    regular1=${white50}
    regular2=${white50}
    regular3=${white}
    regular4=${white25}
    regular5=${white50}
    regular6=${white75}
    regular7=${white75}
    bright0=${nearBlackTinted}
    bright1=${white75}
    bright2=${white75}
    bright3=${white}
    bright4=${white50}
    bright5=${white75}
    bright6=${white}
    bright7=${white}'';

  # The tile's terminal. `os/bin/coder-pane` opens foot running Coder or a
  # bare shell.
  #
  # foot was measured against ghostty, kitty, and alacritty on the nixpkgs
  # revision this flake pins. It is Wayland-only, so it links no X11 or
  # OpenGL stack, and its closure is 96 MB against 233 MB for alacritty,
  # 592 MB for kitty, and 1,045 MB for ghostty. All four draw 24-bit colour;
  # foot was the fastest to open a window. A pane reads
  # `/etc/coderos/foot.ini` below for the Coder palette and the font; a
  # `foot` you type yourself reads your own configuration.
  coderPane = pkgs.writeShellApplication {
    name = "coder-pane";
    runtimeInputs = [ pkgs.foot pkgs.coreutils ];
    text = builtins.readFile ../../bin/coder-pane;
  };

  # The window a bind opens and the window the session opens for itself are
  # the same window.
  openCoder = "${cfg.panePackage}/bin/coder-pane";

  # mako, on the colours in /etc/coderos/mako.ini. The colours go in a file
  # because Hyprland reads `#` on a line of its own configuration as the
  # start of a comment.
  notifications = "${pkgs.mako}/bin/mako --config=/etc/coderos/mako.ini";
  makoConf = ''
    background-color=#${nearBlack}
    text-color=#${white}
    border-color=#${white}
    border-size=1
    border-radius=0
    font=Paper Mono 12
  '';

  # What the session opens at every login: the host's `start` list, then
  # the rows the capability modules add through `capabilityStart`. Keeping
  # the two apart is what lets a module add a row without replacing the
  # default list, because a definition of `start` anywhere drops its
  # default.
  sessionStart = cfg.start ++ cfg.capabilityStart;

  # The start list as Hyprland reads it, one `exec-once` row a line, at the
  # column the text below puts the first one.
  startLines = lib.concatMapStringsSep "\n    " (row: "exec-once = ${row}") sessionStart;

  # Whether a module turned a launcher on. Each module that owns a launcher
  # row of `crates/coder-binds` adds its name to `launchers`, so this file
  # reads no option a module it does not import would declare.
  launches = name: lib.elem name cfg.launchers;

  # The launchers the Coder compositor answers from the table, each named by
  # the `option` field of its row in `crates/coder-binds`. Hyprland gets a
  # `bind` line for each one in the configuration below; the compositor
  # reads the same rows from the table and answers the chords this list
  # names. `crates/coder-binds/tests/binds.rs` keeps this list equal to the
  # table's.
  compositorLaunchers = lib.filter launches [
    "dictation"
    "presentation"
    "browser"
    "android"
    "camera"
    "hands"
  ];

  # A host's own launcher, as the Coder compositor reads it from the grant
  # and `coder_binds::ExtraBind::parse` takes it.
  extraBindGrant = bind: { inherit (bind) mods key command; };

  # A host's own launcher as a Hyprland line. The keyword comes from a
  # variable so that the drift test, which reads every literal bind line in
  # this file as a row of the table, reads only the table's rows.
  bindKeyword = "bind";
  extraBindLine = bind: "${bindKeyword} = ${bind.mods}, ${bind.key}, exec, ${bind.command}";

  # A literal with the characters RE2 reads as syntax escaped, the same set
  # `escape` in `crates/coder-binds/src/rules.rs` escapes.
  escapeRegex = lib.escape (lib.stringToCharacters "\\.+*?()|[]{}^$");

  # One literal of a host's window rule as an alternative of the regular
  # expression Hyprland reads: the whole string, its start, or its start and
  # a word after it.
  patternRegex = pattern:
    if pattern.exact != null then
      escapeRegex pattern.exact
    else if pattern.holds != null then
      "${escapeRegex pattern.prefix}.*${escapeRegex pattern.holds}.*"
    else
      "${escapeRegex pattern.prefix}.*";

  # A host's window rule as the `windowrule` line Hyprland 0.55 reads, in the
  # order `coder_binds::ExtraRule::hyprland` renders the same rule, which a
  # test there holds for the rules `tests/extension-points.nix` sets.
  extraRuleLine = rule:
    let
      flag = lib.optionalString rule.ignoreCase "(?i)";
      alternatives = lib.concatMapStringsSep "|" patternRegex rule.patterns;
      effects =
        lib.optional (rule.float == true) "float on"
        ++ lib.optional rule.center "center on"
        ++ lib.optional rule.keepAspectRatio "keep_aspect_ratio on"
        ++ lib.optional (rule.border != null) "border_size ${toString rule.border}"
        ++ lib.optional (rule.shadow == false) "no_shadow on"
        ++ lib.optional rule.pin "pin on"
        ++ lib.optional (rule.float == false) "tile on"
        ++ lib.optional rule.suppressFullscreen "suppress_event maximize fullscreen";
    in
    lib.concatStringsSep ", " ([ "windowrule = match:${rule.field} ${flag}^(${alternatives})$" ] ++ effects);

  # A host's window rule as the Coder compositor reads it from the grant,
  # into a `coder_binds::ExtraRule`. A pattern carries only the fields it
  # sets.
  extraRuleGrant = rule: {
    inherit (rule) name field ignoreCase;
    patterns = map (lib.filterAttrs (_: value: value != null)) rule.patterns;
    effects = {
      inherit (rule) float center pin keepAspectRatio border shadow suppressFullscreen;
    };
  };

  # The script a login runs for a Coder compositor session. It starts the
  # compositor, reads the two sockets the compositor announces, hands them
  # to the session manager, and opens the start list in it;
  # `os/bin/coder-compositor-session` holds the reasoning.
  compositorSession = pkgs.writeShellApplication {
    name = "coder-compositor-session";
    runtimeInputs = [ pkgs.jq pkgs.coreutils pkgs.gnused pkgs.procps pkgs.uwsm ];
    text = builtins.readFile ../../bin/coder-compositor-session;
  };

  # The compositor the session runs: the package, or the binary a host
  # named instead.
  compositorCommand =
    if cfg.compositorBinary != null then
      cfg.compositorBinary
    else if cfg.compositorPackage != null then
      "${cfg.compositorPackage}/bin/coder-compositor"
    else
      null;

  # What the host granted the Coder compositor's session, which the session
  # script reads. The pane is what its Super+Return row opens; its Super+T
  # row opens the same command with `--shell`. The terminal is what
  # `coder-pane` runs, and the command is what a pane runs.
  compositorGrant = {
    compositor = compositorCommand;
    pane = "${cfg.panePackage}/bin/coder-pane";
    terminal = "${pkgs.foot}/bin/foot";
    command = cfg.command;
    start = sessionStart;
    launchers = compositorLaunchers;
    extraBinds = map extraBindGrant cfg.extraBinds;
    extraRules = map extraRuleGrant cfg.extraWindowRules;
  };

  # What `coder-pane` reads: the palette, the font, and the size. The font
  # is Paper Mono, the one `fonts.packages` below installs. Paper Mono has
  # no braille block, U+2800 to U+28FF, which Coder's spinner draws in;
  # foot draws that block itself rather than from the font, as it does box
  # drawing. A host that wants another size writes its own file and names it
  # in `CODER_PANE_TERMINAL_CONFIG`.
  footConf = ''
    # The terminal coder-pane opens on a CoderOS host.
    # os/modules/coderos/desktop.nix writes this file.

    font=Paper Mono:size=14
    pad=8x8

    # Coder draws one palette on one background, so foot's two themes are
    # the same theme. It starts on the dark one.
    [colors-dark]
    background=${nearBlack}
    foreground=${white}
    ${footPalette}

    [colors-light]
    background=${nearBlack}
    foreground=${white}
    ${footPalette}

    # The size of the text, from the keyboard. Control belongs to the
    # terminal and reaches the window the text is in; Super is the
    # compositor's modifier. These are foot's own defaults, written down
    # so a reader finds them here.
    [key-bindings]
    font-increase=Control+plus Control+equal Control+KP_Add
    font-decrease=Control+minus Control+KP_Subtract
    font-reset=Control+0 Control+KP_0
  '';

  # Whether this host runs the Coder compositor at all: on tty1, or on the
  # trial TTY beside Hyprland. A host that runs neither builds no compositor
  # and writes no grant, so the option costs a Hyprland host nothing.
  runsCoderCompositor = cfg.compositor == "coder" || cfg.trialTty != null;

  # What a login starts for one compositor.
  sessionCommand = which:
    if which == "coder" then
      "${cfg.compositorSessionPackage}/bin/coder-compositor-session"
    else
      "Hyprland --config /etc/coderos/hyprland.conf";

  # What the session calls itself in `XDG_CURRENT_DESKTOP`. The session
  # manager fills that variable from the compositor's own name, and the Coder
  # session's name is the path of a shell script: `uwsm start -n -- <script>`
  # reads back `coder-compositor-session` without this and `Coder` with it.
  # The Hyprland name is the one uwsm reads from that compositor's desktop
  # entry, so naming it changes nothing on a Hyprland host.
  desktopName = which: if which == "coder" then "Coder" else "Hyprland";

  # The trial TTY starts the compositor tty1 does not, so one option swaps the
  # two: the default moves to the other compositor, and the one it replaced
  # keeps running a console key away.
  trialCompositor = if cfg.compositor == "hyprland" then "coder" else "hyprland";

  hyprlandConf = ''
    # The Hyprland session on a CoderOS host. os/modules/coderos/desktop.nix
    # writes this file; edits to it last until the next rebuild.

    monitor = , preferred, auto, 1

    general {
        layout = dwindle
        gaps_in = 3
        gaps_out = 6
        border_size = 1
        col.active_border = rgb(${white})
        col.inactive_border = rgb(${white25})
    }

    dwindle {
        preserve_split = true
    }

    decoration {
        rounding = 0
        blur {
            enabled = false
        }
        shadow {
            enabled = false
        }
    }

    # A tile appears where it lands. Coder redraws the screen on its own
    # schedule, and an animation that moves a terminal costs frames it wants.
    animations {
        enabled = false
    }

    misc {
        disable_hyprland_logo = true
        disable_splash_rendering = true
        force_default_wallpaper = 0
    }

    input {
        kb_layout = us
        follow_mouse = 1
    }

    # What the session opens for itself, the Coder pane first.
    ${startLines}

    # The notification daemon, which draws the close key's notice and the
    # banner that says the microphone is open, in the console white. It
    # starts here rather than from the start list, which the Coder
    # compositor's session also reads, so that session decides for itself
    # what draws a notice.
    exec-once = ${notifications}

    # SUPER + RETURN opens another one, which dwindle tiles beside the
    # first.
    bind = SUPER, RETURN, exec, ${openCoder}

    # The rest is what a tiling session needs to be usable at all. The set
    # and the keys follow Omarchy's, in default/hypr/bindings/tiling.lua, so
    # a person who has used that desktop does not have to learn a second
    # vocabulary to use this one.

    # Close. Both keys, because Omarchy binds both and muscle memory picks
    # one without asking which.
    #
    # The keys run `coder-close` rather than `killactive`. A window whose
    # Coder session has a turn streaming or a delegation that has not
    # reported gets a notice naming what is running, and closes on the press
    # after it. Every other window, and an idle Coder, closes on the first
    # press. os/bin/coder-close says how it decides.
    bind = SUPER, W, exec, ${closeWindow}/bin/coder-close
    bind = SUPER, Q, exec, ${closeWindow}/bin/coder-close

    # Move the focus between tiles.
    bind = SUPER, left, movefocus, l
    bind = SUPER, right, movefocus, r
    bind = SUPER, up, movefocus, u
    bind = SUPER, down, movefocus, d

    # Move between workspaces, and put a window on one.
    bind = SUPER, 1, workspace, 1
    bind = SUPER, 2, workspace, 2
    bind = SUPER, 3, workspace, 3
    bind = SUPER, 4, workspace, 4
    bind = SUPER, 5, workspace, 5
    bind = SUPER, 6, workspace, 6
    bind = SUPER, 7, workspace, 7
    bind = SUPER, 8, workspace, 8
    bind = SUPER, 9, workspace, 9
    bind = SUPER SHIFT, 1, movetoworkspace, 1
    bind = SUPER SHIFT, 2, movetoworkspace, 2
    bind = SUPER SHIFT, 3, movetoworkspace, 3
    bind = SUPER SHIFT, 4, movetoworkspace, 4
    bind = SUPER SHIFT, 5, movetoworkspace, 5
    bind = SUPER SHIFT, 6, movetoworkspace, 6
    bind = SUPER SHIFT, 7, movetoworkspace, 7
    bind = SUPER SHIFT, 8, movetoworkspace, 8
    bind = SUPER SHIFT, 9, movetoworkspace, 9

    # Move the tile itself, which is how a dwindle layout is rearranged.
    bind = SUPER SHIFT, left, movewindow, l
    bind = SUPER SHIFT, right, movewindow, r
    bind = SUPER SHIFT, up, movewindow, u
    bind = SUPER SHIFT, down, movewindow, d

    # Resize the tile. 40 pixels a press is small enough to aim with and
    # large enough to get somewhere.
    #
    # CTRL rather than ALT, which the monitor binds below take. A chord
    # carries one bind: Hyprland runs every bind a press matches, so a chord
    # bound twice resizes the tile and leaves the monitor on the same press.
    # `CKeybindManager::handleKeybinds` collects the matches into `bindsHit`
    # and the loop after it calls each one, and `Hyprland --verify-config`
    # reports `config ok` for such a pair.
    binde = SUPER CTRL, left, resizeactive, -40 0
    binde = SUPER CTRL, right, resizeactive, 40 0
    binde = SUPER CTRL, up, resizeactive, 0 -40
    binde = SUPER CTRL, down, resizeactive, 0 40

    # Monitors, on a host with more than one screen.
    #
    # `movefocus` above crosses a monitor's edge only when a window sits in
    # the direction it casts. A second screen that is empty, or whose windows
    # do not line up with the focused one, leaves the focus where it is and
    # the press does nothing. The dispatchers below name a monitor instead of
    # casting for a window, so they reach a screen whatever it holds.
    #
    # Each one is a no-op on a host with one monitor, and says so:
    # `hyprctl dispatch focusmonitor l` reports `Monitor not found`.

    # Focus another monitor. TAB cycles through them in the order Hyprland
    # holds them, and the arrows aim at the monitor in a direction. CTRL +
    # ALT + TAB is Omarchy's chord for this, in bindings/tiling.lua; SUPER +
    # TAB is a workspace on that desktop.
    bind = CTRL ALT, TAB, focusmonitor, +1
    bind = CTRL ALT SHIFT, TAB, focusmonitor, -1
    bind = SUPER ALT, left, focusmonitor, l
    bind = SUPER ALT, right, focusmonitor, r
    bind = SUPER ALT, up, focusmonitor, u
    bind = SUPER ALT, down, focusmonitor, d

    # Send the focused window to the next monitor over. The `mon:` prefix
    # tells movewindow to read its argument as a monitor, where the same
    # dispatcher on SUPER + SHIFT above reads a direction inside the layout.
    bind = SUPER SHIFT ALT, left, movewindow, mon:l
    bind = SUPER SHIFT ALT, right, movewindow, mon:r
    bind = SUPER SHIFT ALT, up, movewindow, mon:u
    bind = SUPER SHIFT ALT, down, movewindow, mon:d

    # Send the whole desk, with every window on it, to the next monitor over.
    #
    # These two families are where the keys part from Omarchy, which puts the
    # desk on SUPER + SHIFT + ALT and binds no chord for a single window.
    # Here the window takes that chord, beside the SUPER + SHIFT that moves
    # it inside a layout, and the desk takes the longer SUPER + CTRL + ALT.
    bind = SUPER CTRL ALT, left, movecurrentworkspacetomonitor, l
    bind = SUPER CTRL ALT, right, movecurrentworkspacetomonitor, r
    bind = SUPER CTRL ALT, up, movecurrentworkspacetomonitor, u
    bind = SUPER CTRL ALT, down, movecurrentworkspacetomonitor, d

    # Layout. J turns the split the next window opens on; SPACE / D toggles
    # a window between floating and tiled. T opens a pane running a shell.
    # togglesplit is dwindle's, not a dispatcher of its own, so it goes
    # through layoutmsg. Hyprland 0.55 rejects the bare form.
    bind = SUPER, J, layoutmsg, togglesplit
    bind = SUPER, SPACE, togglefloating
    bind = SUPER, D, togglefloating
    bind = SUPER, T, exec, ${cfg.panePackage}/bin/coder-pane --shell

    # Fullscreen, in both of Hyprland's senses: F takes the whole screen,
    # and CTRL+F fills the tiling area and leaves the rest of the layout in
    # place, which is what a monocle mode is.
    bind = SUPER, F, fullscreen, 0
    bind = SUPER CTRL, F, fullscreen, 1

    # Push to talk, when dictation is on. One press records from the named
    # microphone, the next transcribes it and types the text into whatever
    # has focus. While it records, a notification says the microphone is
    # open, because the tone that used to be the only cue fails silently
    # whenever output is muted or busy. The key belongs here rather than in
    # a shell's rc file: a key added by `hyprctl keyword` from ~/.bashrc runs
    # on every shell including the non-interactive ones scp and rsync open,
    # where it prints "HYPRLAND_INSTANCE_SIGNATURE not set!" onto stdout and
    # breaks the transfer.
${lib.optionalString (launches "dictation") "    bind = SUPER, V, exec, dictate-toggle\n"}
    # Presentation mode, on one key. It sets the screen up to be watched and
    # writes down what the desktop was; the same key puts the desktop back
    # from what it wrote. os/bin/presentation-mode says what is in that set.
${lib.optionalString (launches "presentation") "    bind = SUPER, P, exec, presentation-mode toggle\n"}
    # The Coder Browser. os/modules/coderos/browser.nix holds the option.
${lib.optionalString (launches "browser") "    bind = SUPER, B, exec, coder-browser\n"}
    # The Android emulator, when the host has one. It floats: a phone is
    # tall and narrow, and a tile beside a terminal on a wide monitor would
    # show it letterboxed in a column. `keep_aspect_ratio` holds the phone's
    # shape under a mouse resize, and SUPER with the left button moves it.
    # `hyprctl dispatch togglefloating` puts it in the layout for a person
    # who wants it there. The window is an X11 one, so the class is what
    # os/modules/coderos/android.nix says the QEMU process announces, and
    # `force_zero_scaling` keeps it drawing at 1x under presentation mode's
    # scale, sharp and smaller rather than upscaled and blurred.
${lib.optionalString (launches "android") ''
    bind = SUPER, A, exec, android-emulator
    windowrule = match:class ^(${cfg.android.windowClass})$, float on, keep_aspect_ratio on
    xwayland {
        force_zero_scaling = true
    }
''}
    # The camera view, on a host whose camera module adds `camera` to the
    # launchers. The circle is in the picture: the camera overlay masks the
    # camera to a disc on a transparent background, so the window is a plain
    # rectangle that floats, stays square under a mouse resize, draws no
    # border around its clear margins, and is pinned so it follows across
    # desks. SUPER + C turns the camera view on and off.
    #
    # The syntax is Hyprland 0.55's: one `windowrule` line of `match:` fields
    # and effects, and `windowrulev2` is an error the compositor shows on
    # every start. `Hyprland --verify-config -c <file>` checks a file without
    # a session, and 0.55.4 answers `config ok` to this line.
${lib.optionalString (launches "camera") ''
    windowrule = match:title ^(selfie)$, float on, keep_aspect_ratio on, border_size 0, no_shadow on, pin on
    bind = SUPER, C, exec, camera-toggle
''}
    # The recording HUD docks under the camera circle, so it needs a camera to
    # sit under and the recorder its button drives. The strip is a floating,
    # borderless, pinned panel like the camera.
${lib.optionalString (launches "camera" && cfg.screenRecording.enable) ''
    windowrule = match:title ^(recording-hud)$, float on, border_size 0, no_shadow on, pin on
''}
    # The host's own launchers and window rules, from `extraBinds` and
    # `extraWindowRules`, which modules in a host's own flake set.
    ${lib.concatMapStringsSep "\n    " extraBindLine cfg.extraBinds}
    ${lib.concatMapStringsSep "\n    " extraRuleLine cfg.extraWindowRules}

    # Drag a floating window with the mouse, and resize it with the right
    # button. Without these a floating window can only be moved by a
    # dispatcher, which is no use to a person who wants a float somewhere
    # else on the screen.
    bindm = SUPER, mouse:272, movewindow
    bindm = SUPER, mouse:273, resizewindow

    bind = SUPER SHIFT, E, exit
  '';

  # The close key. `os/bin/coder-close` holds what it decides and why; the
  # short version is that a window whose Coder session has work in flight
  # takes a notice and a second press, and every other window closes on the
  # first press. The script is a real file under os/bin so it can be read and
  # linted as shell, and writeShellApplication supplies the shebang, sets the
  # shell options, runs shellcheck at build time, and puts the runtime tools
  # on PATH by name. The client it asks is `command`, reached through the
  # inherited PATH the way a pane reaches it.
  closeWindow = pkgs.writeShellApplication {
    name = "coder-close";
    runtimeInputs = [
      cfg.deskPackage
      pkgs.jq
      pkgs.libnotify
      pkgs.gawk
      pkgs.coreutils
      pkgs.procps
    ];
    runtimeEnv.CODER_CLOSE_CLIENT = cfg.command;
    text = builtins.readFile ../../bin/coder-close;
  };

  # One launcher a host adds. The fields are spelled the way a Hyprland
  # `bind` line spells them.
  extraBind = lib.types.submodule {
    options = {
      mods = lib.mkOption {
        type = lib.types.strMatching "(SUPER|SHIFT|CTRL|ALT)( (SUPER|SHIFT|CTRL|ALT))*";
        default = "SUPER";
        example = "SUPER SHIFT";
        description = "The modifiers, as a Hyprland `bind` line spells them.";
      };
      key = lib.mkOption {
        type = lib.types.str;
        example = "G";
        description = ''
          The key: a letter, a digit, `RETURN`, `SPACE`, `TAB`, or an arrow
          (`left`, `right`, `up`, `down`).
        '';
      };
      command = lib.mkOption {
        type = lib.types.str;
        example = "coder-battlenet";
        description = "The command line the chord runs, through a shell.";
      };
    };
  };

  # One literal a host's window rule accepts.
  rulePattern = lib.types.submodule {
    options = {
      exact = lib.mkOption {
        type = lib.types.nullOr lib.types.str;
        default = null;
        description = "The whole string.";
      };
      prefix = lib.mkOption {
        type = lib.types.nullOr lib.types.str;
        default = null;
        description = "The start of the string.";
      };
      holds = lib.mkOption {
        type = lib.types.nullOr lib.types.str;
        default = null;
        description = "With `prefix`, a word somewhere after the start.";
      };
    };
  };

  # One window rule a host adds, in the shape of a `coder_binds::Rule`.
  extraRule = lib.types.submodule {
    options = {
      name = lib.mkOption {
        type = lib.types.str;
        description = "What the rule is for, in a few words.";
      };
      field = lib.mkOption {
        type = lib.types.enum [ "class" "title" ];
        default = "class";
        description = ''
          Which string of a window the rule reads: `class`, the app-id or an
          X11 window's class, or `title`.
        '';
      };
      patterns = lib.mkOption {
        type = lib.types.nonEmptyListOf rulePattern;
        description = ''
          The literals the rule accepts. A window matches when any one does.
          Each sets `exact`, or `prefix` with an optional `holds`.
        '';
      };
      ignoreCase = lib.mkOption {
        type = lib.types.bool;
        default = false;
        description = ''
          Whether ASCII case is ignored. Wine reports a class as the
          executable's name in whichever case the launcher spelled it.
        '';
      };
      float = lib.mkOption {
        type = lib.types.nullOr lib.types.bool;
        default = null;
        description = ''
          True floats the window over the layout, false puts it in a tile,
          and null leaves it where the layout put it.
        '';
      };
      center = lib.mkOption {
        type = lib.types.bool;
        default = false;
        description = "Whether a float opens in the middle of the screen.";
      };
      pin = lib.mkOption {
        type = lib.types.bool;
        default = false;
        description = "Whether the window shows on every desk.";
      };
      keepAspectRatio = lib.mkOption {
        type = lib.types.bool;
        default = false;
        description = "Whether a resize keeps the ratio the window mapped at.";
      };
      border = lib.mkOption {
        type = lib.types.nullOr lib.types.ints.unsigned;
        default = null;
        description = "The border's thickness in pixels. 0 draws none.";
      };
      shadow = lib.mkOption {
        type = lib.types.nullOr lib.types.bool;
        default = null;
        description = "Whether the window casts a shadow. Null leaves the session's default.";
      };
      suppressFullscreen = lib.mkOption {
        type = lib.types.bool;
        default = false;
        description = ''
          Whether a fullscreen or maximize request from the client leaves
          the window where the layout has it.
        '';
      };
    };
  };

  # Every chord the configuration binds, as its sorted modifiers and its key,
  # once for each line that binds it. A chord that shows up twice runs two
  # actions on one press, because Hyprland runs every bind a press matches.
  # The keywords come from `bindKeyword` for the drift test's sake, as in
  # `extraBindLine`.
  bindPrefixes = [ "${bindKeyword} = " "${bindKeyword}e = " ];
  chordOf = line:
    let
      body = lib.foldl' (text: prefix: lib.removePrefix prefix text) line bindPrefixes;
      parts = lib.splitString ", " body;
      mods = lib.filter (word: word != "") (lib.splitString " " (lib.toUpper (lib.elemAt parts 0)));
    in
    "${lib.concatStringsSep "+" (lib.sort lib.lessThan mods)}+${lib.toUpper (lib.elemAt parts 1)}";
  boundChords = map chordOf (lib.filter
    (line: lib.any (prefix: lib.hasPrefix prefix line) bindPrefixes)
    (map lib.trim (lib.splitString "\n" hyprlandConf)));
  doubledChords = lib.unique
    (lib.filter (chord: lib.count (other: other == chord) boundChords > 1) boundChords);

  # The patterns that set neither `exact` nor `prefix`, both, or `holds`
  # without `prefix`, by the rule that holds them.
  badPatterns = lib.concatMap
    (rule: lib.optional
      (lib.any
        (pattern: (pattern.exact == null) == (pattern.prefix == null)
          || (pattern.holds != null && pattern.prefix == null))
        rule.patterns)
      rule.name)
    cfg.extraWindowRules;
in
{
  options.coderos.desktop = {
    enable = lib.mkEnableOption "a Hyprland session whose default window is Coder";

    user = lib.mkOption {
      type = lib.types.str;
      description = ''
        The account the session runs as. The console login on tty1 starts the
        compositor, so this has to be the account
        `services.getty.autologinUser` names. Optional capabilities such as
        the Android emulator grant their devices and paths to this account.
      '';
    };

    # A string, not a package: `coder-new` is built from a checkout of this
    # repository and installed on the session's PATH.
    # `environment.localBinInPath` puts ~/.local/bin on the login shell's
    # PATH, and the session inherits it from the login shell, so a bare
    # command name resolves there.
    command = lib.mkOption {
      type = lib.types.str;
      default = "coder-new";
      description = ''
        The Coder command a pane runs, and the one the close key asks what is
        running. The default is this repository's `coder-new`, found on the
        session's PATH.
      '';
    };

    panePackage = lib.mkOption {
      type = lib.types.package;
      default = coderPane;
      defaultText = lib.literalExpression "the `coder-pane` script in os/bin";
      description = ''
        The `coder-pane` command every bind opens a window with. It opens the
        terminal emulator in `CODER_PANE_TERMINAL` running `command`.
      '';
    };

    directory = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
      example = "/srv/checkouts/openagents";
      description = ''
        The directory the session's own window and every SUPER + RETURN
        window open in. Null leaves the pane where the session starts it,
        which is the account's home.

        Point this at a checkout. A writing delegation takes a lock on the
        directory it writes in, and outside a repository there is no
        worktree to give the next one, so every writing delegation on the
        host queues behind one lock and only one ever runs. In a checkout
        each writer gets a worktree of its own and they run beside each
        other.
      '';
    };

    compositor = lib.mkOption {
      type = lib.types.enum [ "hyprland" "coder" ];
      default = "hyprland";
      description = ''
        The compositor the tty1 login starts under the session manager:
        Hyprland, or the Coder compositor that `crates/coder-compositor`
        builds. The option defaults to Hyprland.

        Read this option with `trialTty` below, which runs the other
        compositor on a second TTY: beside a Hyprland session that TTY is the
        trial, and beside a Coder session it is the way back.
      '';
    };

    trialTty = lib.mkOption {
      type = lib.types.nullOr lib.types.ints.positive;
      default = null;
      example = 2;
      description = ''
        A second TTY whose login starts the compositor tty1 does not, so a
        host runs both at once and you switch between them with the console
        keys. Null starts nothing but the tty1 session.

        On a host whose tty1 runs the Coder compositor, this TTY is the way
        back: it runs Hyprland with the configuration this module writes, and
        Ctrl+Alt with the TTY's function key reaches it without a rebuild or
        a reboot.

        The second login starts its compositor without the session manager.
        One account has one graphical session, so `uwsm check may-start`
        refuses a second while the tty1 session holds it. What that session
        gives up is the portals and the user units that key off
        `graphical-session.target`; the screen, the keys, the windows, and
        the start list are the compositor's own and work without them.
      '';
    };

    start = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [ openCoder ];
      defaultText = lib.literalExpression ''[ "''${panePackage}/bin/coder-pane" ]'';
      description = ''
        What the session opens for itself at every login, as command lines.
        Hyprland reads the list as `exec-once` rows, and the Coder
        compositor's session reads the same rows from its grant.

        The default is the Coder window. Name the whole list to change it.
        The capability modules add their own rows after this list, through
        `capabilityStart`, so naming it keeps theirs.
      '';
    };

    capabilityStart = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [ ];
      internal = true;
      description = ''
        The command lines the capability modules of this flake open at every
        login, after `start`. Each module adds its rows when its option is
        on, and `lib.mkBefore` or `lib.mkAfter` orders one module's rows
        against another's. The session opens them in the list's order.
      '';
    };

    launchers = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [ ];
      internal = true;
      description = ''
        The launcher rows of `crates/coder-binds` that the modules of this
        flake turned on, by the row's `option` field. Each module that owns
        a row adds its name here when its option is on, and this file writes
        the row's bind and window rules for the names it finds.
      '';
    };

    extraBinds = lib.mkOption {
      type = lib.types.listOf extraBind;
      default = [ ];
      example = lib.literalExpression ''
        [ { mods = "SUPER"; key = "G"; command = "coder-battlenet"; } ]
      '';
      description = ''
        Launcher chords a host adds beside the ones this flake's modules
        carry, for a module in the host's own flake. Hyprland gets a `bind`
        line for each, and the Coder compositor reads them from its grant as
        `extraBinds`. A chord this flake already binds is refused, because
        Hyprland would run both actions on one press.
      '';
    };

    extraWindowRules = lib.mkOption {
      type = lib.types.listOf extraRule;
      default = [ ];
      example = lib.literalExpression ''
        [
          {
            name = "StarCraft II client, by title";
            field = "title";
            patterns = [ { prefix = "StarCraft II"; } ];
            ignoreCase = true;
            float = false;
            suppressFullscreen = true;
          }
        ]
      '';
      description = ''
        Window rules a host adds beside the ones this flake's modules carry,
        for the windows a module in the host's own flake opens. Hyprland gets
        a `windowrule` line for each, and the Coder compositor reads them
        from its grant as `extraRules` and applies them after its own.
      '';
    };

    compositorPackage = lib.mkOption {
      type = lib.types.nullOr lib.types.package;
      default = pkgs.callPackage ../../pkgs/coder-compositor.nix { };
      defaultText = lib.literalExpression "pkgs.callPackage ../../pkgs/coder-compositor.nix { }";
      description = ''
        The Coder compositor the session runs, whose `bin/coder-compositor`
        the grant names, built from this repository
        (`nix build ./os#coder-compositor`). A host that runs Hyprland and
        names no trial TTY builds none of it.
      '';
    };

    compositorSessionPackage = lib.mkOption {
      type = lib.types.nullOr lib.types.package;
      default = compositorSession;
      defaultText = lib.literalExpression "the `coder-compositor-session` script in os/bin";
      description = ''
        The script a login runs for a Coder compositor session, as
        `bin/coder-compositor-session`. It starts the compositor, hands its
        sockets to the session manager, and opens the start list from the
        grant at /etc/coderos/compositor.json.
      '';
    };

    compositorBinary = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
      example = "/srv/checkouts/openagents/target/release/coder-compositor";
      description = ''
        The absolute path of a compositor built from a checkout, as a
        string, which the session runs instead of `compositorPackage`. It is
        a string rather than a path for the reason `command` is: a path
        would copy the binary into the Nix store, and this points at a build
        artifact.
      '';
    };
  };

  config = lib.mkIf cfg.enable {
    programs.hyprland = {
      enable = true;
      # A pane is a Wayland window. A module whose window is an X11 one, such
      # as the Android emulator, turns Xwayland on.
      xwayland.enable = lib.mkDefault false;
      # Start under the session manager. Hyprland warns on a bare start, and
      # the warning is earned: without it the compositor is a process on a
      # login shell rather than a registered graphical session, so logind
      # does not know a session exists, the portals and the systemd user
      # units that key off `graphical-session.target` do not run, and nothing
      # tears the session down in order when it ends.
      withUWSM = true;
    };

    hardware.graphics.enable = lib.mkDefault true;

    # Hyprland resolves this path at launch. As a symlink into the store, the
    # path resolved to one generation's immutable file, so a running session
    # held the old store path and `hyprctl reload` re-read it rather than the
    # new one; on 2026-09-10 a switch that added two binds changed nothing in
    # the session that was up. `mode` makes it a regular file the activation
    # rewrites in place, so the path a session resolved at launch is the path
    # a switch updates, and the activation script below asks every running
    # session to reload it.
    environment.etc."coderos/hyprland.conf" = {
      text = hyprlandConf;
      mode = "0444";
    };

    # Tell each running session to re-read its configuration after a switch.
    # The instance sockets sit under /run/user/<uid>/hypr/<signature>/, and
    # the desk command finds one through the two variables the session
    # announces. Nothing here fails the activation: a host with no session up
    # has nothing to reload.
    system.activationScripts.coderosHyprlandReload = lib.stringAfter [ "etc" ] (''
      for dir in /run/user/*/hypr/*/; do
        [ -S "$dir.socket.sock" ] || continue
        runtime=''${dir%/hypr/*}
        signature=$(basename "$dir")
        XDG_RUNTIME_DIR="$runtime" HYPRLAND_INSTANCE_SIGNATURE="$signature" \
          ${cfg.deskPackage}/bin/coder-desk reload >/dev/null 2>&1 || true
      done
    '' + lib.optionalString runsCoderCompositor ''
      for socket in /run/user/*/coder-desk/*.sock; do
        [ -S "$socket" ] || continue
        CODER_DESK_SOCKET="$socket" \
          ${cfg.deskPackage}/bin/coder-desk reload >/dev/null 2>&1 || true
      done
    '');

    # What the Coder compositor's session reads, on a host that runs it.
    environment.etc."coderos/compositor.json" =
      lib.mkIf runsCoderCompositor { text = builtins.toJSON compositorGrant; };

    # What `coder-pane` reads, on either compositor.
    environment.etc."coderos/foot.ini".text = footConf;
    environment.etc."coderos/mako.ini".text = makoConf;

    # The proprietary NVIDIA driver keeps the buffers a compositor frees in
    # a pool of its own rather than handing them back, and a compositor that
    # reallocates its swapchain holds about a gigabyte of video memory where
    # it needs about 100 megabytes. `niri` documents this application profile
    # as the fix. It matches the process by name: `coder-compositor` for a
    # build from a checkout, and `.coder-compositor-wrapped` for a package
    # whose wrapper runs the binary under that name.
    environment.etc."nvidia/nvidia-application-profiles-rc.d/50-coder-compositor.json" =
      lib.mkIf (runsCoderCompositor && lib.elem "nvidia" config.services.xserver.videoDrivers) {
        text = builtins.toJSON {
          rules = map
            (name: {
              pattern = {
                feature = "procname";
                matches = name;
              };
              profile = "Limit free buffer pool in the Coder compositor";
            })
            [ "coder-compositor" ".coder-compositor-wrapped" ];
          profiles = [
            {
              name = "Limit free buffer pool in the Coder compositor";
              settings = [
                {
                  key = "GLVidHeapReuseRatio";
                  value = 0;
                }
              ];
            }
          ];
        };
      };

    # foot is what `coder-pane` runs. libnotify is the notice the close key
    # raises; the key carries its own copy through `runtimeInputs`, and the
    # session gets one so a person can raise a notification by hand while
    # testing.
    environment.systemPackages =
      [ cfg.panePackage pkgs.foot closeWindow pkgs.libnotify ]
      ++ lib.optionals (runsCoderCompositor && cfg.compositorSessionPackage != null) [ cfg.compositorSessionPackage ]
      ++ lib.optionals (runsCoderCompositor && cfg.compositorBinary == null && cfg.compositorPackage != null) [ cfg.compositorPackage ];

    # Paper Mono is the one typeface CoderOS draws: the only font package
    # the desktop installs, and fontconfig's default for every generic
    # family, so a program that asks for sans or serif gets it too.
    fonts.packages = [ paperMono ];
    fonts.fontconfig.defaultFonts = {
      monospace = [ "Paper Mono" ];
      sansSerif = [ "Paper Mono" ];
      serif = [ "Paper Mono" ];
    };

    # What `os/bin/coder-pane` reads. The client is a command name rather
    # than a package for the reason `coderos.desktop.command` is.
    environment.sessionVariables = {
      CODER_PANE_CLIENT = cfg.command;
      CODER_PANE_TERMINAL = "${pkgs.foot}/bin/foot";
    } // lib.optionalAttrs (cfg.directory != null) {
      CODER_PANE_DIRECTORY = cfg.directory;
    };

    # How the session starts, and why it does not fight the console.
    #
    # tty1 already autologins (`services.getty.autologinUser`), so the login
    # shell starts the compositor rather than a display manager taking the
    # tty from getty. One mechanism owns tty1, the session inherits the login
    # shell's PATH, and a rebuild that lands this change restarts no login
    # that is open.
    #
    # The compositor runs without `exec`, so a session that cannot start
    # leaves you at the shell that started it instead of ending the login and
    # sending getty around the loop again.
    #
    # `uwsm check may-start` is the session manager's own guard: it refuses
    # when a session already runs or the tty is not one it should take. The
    # tty and account tests stay in front of it because they say what this
    # host means rather than what uwsm permits, and because they are cheap.
    #
    # The second TTY starts the other compositor with no session manager in
    # front of it. That guard refuses a second session for one account, which
    # is what it is for, so a second login that asked would be refused while
    # the tty1 session runs.
    #
    # `uwsm start` reads the name the session goes by from the compositor it
    # is given, so a session whose compositor is a script is named for the
    # script. `-D` names it instead, and `XDG_CURRENT_DESKTOP` says `Coder`
    # or `Hyprland` on either default.
    #
    # A second login that starts the Coder compositor sets
    # `CODER_COMPOSITOR_RESTART` for the session script, so the quit chord
    # brings a fresh compositor up within a few seconds rather than leaving a
    # prompt under the last session's output, and the pause between runs is
    # where Ctrl+C keeps the shell. Hyprland reads nothing of that variable,
    # so it goes only to the compositor that reads it.
    programs.bash.loginShellInit = ''
      if [ "$(tty)" = "/dev/tty1" ] && [ -z "$WAYLAND_DISPLAY" ] && [ "$USER" = "${cfg.user}" ] \
        && ${pkgs.uwsm}/bin/uwsm check may-start >/dev/null 2>&1; then
        ${pkgs.uwsm}/bin/uwsm start -D ${desktopName cfg.compositor} -- ${sessionCommand cfg.compositor}
      fi
    '' + lib.optionalString (cfg.trialTty != null) ''
      if [ "$(tty)" = "/dev/tty${toString cfg.trialTty}" ] && [ -z "$WAYLAND_DISPLAY" ] \
        && [ "$USER" = "${cfg.user}" ]; then
        ${lib.optionalString (trialCompositor == "coder") "CODER_COMPOSITOR_RESTART=1 "}${sessionCommand trialCompositor}
      fi
    '';

    assertions = [
      {
        assertion = cfg.trialTty != 1;
        message = ''
          coderos.desktop.trialTty is 1, which is the TTY the session already
          starts on. Name another TTY, such as 2, or leave the option unset.
        '';
      }
      {
        assertion = config.services.getty.autologinUser == cfg.user;
        message = ''
          coderos.desktop.user is "${cfg.user}", and services.getty.autologinUser
          is ${if config.services.getty.autologinUser == null then "unset" else "\"${config.services.getty.autologinUser}\""}.
          The console login on tty1 is what starts the session, so the two name
          the same account.
        '';
      }
      {
        assertion = !runsCoderCompositor
          || (compositorCommand != null && cfg.compositorSessionPackage != null);
        message = ''
          coderos.desktop runs the Coder compositor, because compositor is
          "coder" or trialTty is set, and ${if compositorCommand == null then "neither compositorPackage nor compositorBinary" else "not compositorSessionPackage"} is set.
          Leave those options at their defaults, or set compositor to
          "hyprland" and leave trialTty unset.
        '';
      }
      {
        assertion = doubledChords == [ ];
        message = ''
          coderos.desktop binds ${lib.concatStringsSep ", " doubledChords} more
          than once. An entry of extraBinds takes a chord this flake or another
          entry already binds, and Hyprland would run both on one press. Pick
          another key.
        '';
      }
      {
        assertion = badPatterns == [ ];
        message = ''
          coderos.desktop.extraWindowRules has a pattern that sets neither or
          both of `exact` and `prefix`, or `holds` without `prefix`, in:
          ${lib.concatStringsSep ", " badPatterns}.
        '';
      }
    ];
  };
}
