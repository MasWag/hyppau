#!/usr/bin/env bats

@test "Naive" {
  run cargo run -- -m naive -f ./examples/small.json -i ./examples/small1.txt -i ./examples/small2.txt 
  [ "$status" -eq 0 ]
  trimmed_output="$(echo "$output" | awk '/completed/{f=0} f; /Start/{f=1}')"
  [ "$(echo "$trimmed_output" | xargs)" = "$(xargs < ./examples/small-naive.expected)" ]
}

@test "Online" {
  run cargo run -- -m online -f ./examples/small.json -i ./examples/small1.txt -i ./examples/small2.txt 
  [ "$status" -eq 0 ]
  trimmed_output="$(echo "$output" | awk '/completed/{f=0} f; /Start/{f=1}' | xargs -n 6 | sort -n)"
  [ "$trimmed_output" = "$(xargs -n 6 < ./examples/small-online.expected | sort -n)" ]
}

@test "FJS" {
  run cargo run -- -m fjs -f ./examples/small.json -i ./examples/small1.txt -i ./examples/small2.txt 
  [ "$status" -eq 0 ]
  trimmed_output="$(echo "$output" | awk '/completed/{f=0} f; /Start/{f=1}')"
  [ "$(echo "$trimmed_output" | xargs)" = "$(xargs < ./examples/small-fjs.expected)" ]
}
