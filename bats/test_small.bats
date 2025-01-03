#!/usr/bin/env bats

setup() {
    PROJECT_ROOT="${BATS_TEST_DIRNAME}/.."
    EXAMPLE_DIR="${PROJECT_ROOT}/examples"
}

@test "Naive" {
    cd "$PROJECT_ROOT"
    run cargo run -- -m naive -f "${EXAMPLE_DIR}/small.json" -i "${EXAMPLE_DIR}/small1.txt" -i "${EXAMPLE_DIR}/small2.txt" -o "${BATS_TMPDIR}/small-naive.txt"
    [ "$status" -eq 0 ]

    sort "${BATS_TMPDIR}/small-naive.txt" | uniq > "${BATS_TMPDIR}/small-naive.txt.sorted"
    diff "${BATS_TMPDIR}/small-naive.txt.sorted" "${EXAMPLE_DIR}/small.expected"
}

@test "Online" {
    cd "$PROJECT_ROOT"
    run cargo run -- -m online -f "${EXAMPLE_DIR}/small.json" -i "${EXAMPLE_DIR}/small1.txt" -i "${EXAMPLE_DIR}/small2.txt" -o "${BATS_TMPDIR}/small-online.txt"
    [ "$status" -eq 0 ]

    sort "${BATS_TMPDIR}/small-online.txt" | uniq > "${BATS_TMPDIR}/small-online.txt.sorted"
    diff "${BATS_TMPDIR}/small-online.txt.sorted" "${EXAMPLE_DIR}/small.expected"
}

@test "FJS" {
    cd "$PROJECT_ROOT"
    run cargo run -- -m fjs -f "${EXAMPLE_DIR}/small.json" -i "${EXAMPLE_DIR}/small1.txt" -i "${EXAMPLE_DIR}/small2.txt"  -o "${BATS_TMPDIR}/small-fjs.txt"
    [ "$status" -eq 0 ]

    sort "${BATS_TMPDIR}/small-fjs.txt" | uniq > "${BATS_TMPDIR}/small-fjs.txt.sorted"
    diff "${BATS_TMPDIR}/small-fjs.txt.sorted" "${EXAMPLE_DIR}/small.expected"
}

@test "NaiveFiltered" {
    cd "$PROJECT_ROOT"
    run cargo run -- -m naive-filtered -f "${EXAMPLE_DIR}/small.json" -i "${EXAMPLE_DIR}/small1.txt" -i "${EXAMPLE_DIR}/small2.txt" -o "${BATS_TMPDIR}/small-naive-filtered.txt"
    [ "$status" -eq 0 ]

    sort "${BATS_TMPDIR}/small-naive-filtered.txt" | uniq > "${BATS_TMPDIR}/small-naive-filtered.txt.sorted"
    diff "${BATS_TMPDIR}/small-naive-filtered.txt.sorted" "${EXAMPLE_DIR}/small.expected"
}

@test "OnlineFiltered" {
    cd "$PROJECT_ROOT"
    run cargo run -- -m online-filtered -f "${EXAMPLE_DIR}/small.json" -i "${EXAMPLE_DIR}/small1.txt" -i "${EXAMPLE_DIR}/small2.txt" -o "${BATS_TMPDIR}/small-online-filtered.txt"
    [ "$status" -eq 0 ]

    sort "${BATS_TMPDIR}/small-online-filtered.txt" | uniq > "${BATS_TMPDIR}/small-online-filtered.txt.sorted"
    diff "${BATS_TMPDIR}/small-online-filtered.txt.sorted" "${EXAMPLE_DIR}/small.expected"
}

@test "FJSFiltered" {
    cd "$PROJECT_ROOT"
    run cargo run -- -m fjs-filtered -f "${EXAMPLE_DIR}/small.json" -i "${EXAMPLE_DIR}/small1.txt" -i "${EXAMPLE_DIR}/small2.txt"  -o "${BATS_TMPDIR}/small-fjs-filtered.txt"
    [ "$status" -eq 0 ]

    sort "${BATS_TMPDIR}/small-fjs-filtered.txt" | uniq > "${BATS_TMPDIR}/small-fjs-filtered.txt.sorted"
    diff "${BATS_TMPDIR}/small-fjs-filtered.txt.sorted" "${EXAMPLE_DIR}/small.expected"
}
