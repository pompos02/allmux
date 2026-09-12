#!/usr/bin/env bash

pid="$(pgrep -n -x allmux)"

exec gdb -q -tui "build/debug/allmux" \
  -ex "set scheduler-locking off" \
  -ex "attach $pid" \
  -ex "break /src/application.cpp:206"
