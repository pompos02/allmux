#!/usr/bin/env bash

pid="$(pgrep -n -x allmux)"

exec gdb -q -tui "build/debug/allmux" \
  -ex "attach $pid" \
  -ex "set scheduler-locking off" \
  -ex "break src/tmux.hpp:86"
