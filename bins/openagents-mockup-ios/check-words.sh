#!/usr/bin/env bash
# Fail when an on-screen string in the mockup uses a word the spec bans from
# primary surfaces (docs/product/2026-09-28-app-wireframe.md, "Words on
# screen"), plus "practice" (revision 3 says "test").
#
# It reads every Swift string literal in App/ that could reach the screen: a
# literal with a space or a capital letter, not an SF Symbol name (icon:,
# systemName:, systemImage:), not a #Preview title, and not in a comment.
# Skipped: App/Screens/Retired/ (revision 2, kept for comparison) and the
# Screen index (a design tool). Approved exceptions are listed below with
# the spec's reason.
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

files=$(find "$here/App" -name '*.swift' -not -path '*/Retired/*' -not -name 'ScreenIndex.swift' | sort)

# shellcheck disable=SC2086
perl -e '
use strict; use warnings;
# Case-insensitive word starts (so "keys" and "evaluation" match too).
my @banned = qw(npub nsec key relay nostr ATIF tailnet tailscale wasm plugin extension
  benchmark terminal-bench eval suite case grader rubric judge baseline arm harness
  stand-in mock pilot jev luna microcoder verifier trace recipe grant sats BTC
  lightning invoice host workspace pubkey hex practice);
# Case-sensitive whole words.
my @exact = qw(NIP TB);
# Approved exceptions: exact literal => why.
my %allowed = (
  "Identity keys" => "CHAT-8 screen chip label named by the spec (SCR-17.E06)",
  "\x{20bf} 0.00012" => "balance pill, off by default (MockData.showBalancePill)",
);
my $bad = 0;
for my $file (@ARGV) {
  open(my $fh, "<:encoding(UTF-8)", $file) or die "$file: $!";
  my $n = 0;
  while (my $line = <$fh>) {
    $n++;
    next if $line =~ /^\s*#Preview/;
    my @lits;
    while ($line =~ /\G(?:(icon|systemName|systemImage|image)\s*:\s*"(?:[^"\\]|\\.)*"|"((?:[^"\\]|\\.)*)"|(\/\/.*)|.)/gcs) {
      last if defined $3;
      push @lits, $2 if defined $2;
    }
    for my $lit (@lits) {
      (my $text = $lit) =~ s/\\\((?:[^()]|\([^()]*\))*\)/ /g;   # drop \(interpolations)
      next unless $text =~ /[ A-Z]/;
      next if exists $allowed{$text};
      for my $w (@banned) {
        if ($text =~ /(?<![A-Za-z])\Q$w\E/i) { print "$file:$n: \"$w\" in \"$lit\"\n"; $bad = 1; }
      }
      for my $w (@exact) {
        if ($text =~ /\b\Q$w\E\b/) { print "$file:$n: \"$w\" in \"$lit\"\n"; $bad = 1; }
      }
    }
  }
}
if ($bad) { print "Banned words on screen (see docs/product/2026-09-28-app-wireframe.md, Words on screen).\n"; exit 1 }
print "check-words: no banned words in on-screen strings.\n";
' $files
