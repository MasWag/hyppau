#!/usr/bin/env bats

setup() {
    PROJECT_ROOT="${BATS_TEST_DIRNAME}/.."
    EXAMPLE_DIR="${PROJECT_ROOT}/examples"
    # Generate the input string
    "${EXAMPLE_DIR}/network_pair/gen_log.sh" 1000 > "${BATS_TMPDIR}/network_pair-1000.input"
}

@test "Compare the result of Naive and FJS" {
    cd "$PROJECT_ROOT"

    run cargo run --release -- -f "${EXAMPLE_DIR}/network_pair/network_pair.json" -i "${BATS_TMPDIR}/network_pair-1000.input" -m naive -o "${BATS_TMPDIR}/network_pair-1000.naive"
    run cargo run --release -- -f "${EXAMPLE_DIR}/network_pair/network_pair.json" -i "${BATS_TMPDIR}/network_pair-1000.input" -m fjs -o "${BATS_TMPDIR}/network_pair-1000.fjs"

    sort "${BATS_TMPDIR}/network_pair-1000.naive" | uniq > "${BATS_TMPDIR}/network_pair-1000.naive.sorted"
    sort "${BATS_TMPDIR}/network_pair-1000.fjs" | uniq > "${BATS_TMPDIR}/network_pair-1000.fjs.sorted"

    diff "${BATS_TMPDIR}/network_pair-1000.naive.sorted" "${BATS_TMPDIR}/network_pair-1000.fjs.sorted"

    [ $status -eq 0 ]
}

@test "Compare the result of Naive and Online" {
    cd "$PROJECT_ROOT"

    run cargo run --release -- -f "${EXAMPLE_DIR}/network_pair/network_pair.json" -i "${BATS_TMPDIR}/network_pair-1000.input" -m naive -o "${BATS_TMPDIR}/network_pair-1000.naive"
    run cargo run --release -- -f "${EXAMPLE_DIR}/network_pair/network_pair.json" -i "${BATS_TMPDIR}/network_pair-1000.input" -m online -o "${BATS_TMPDIR}/network_pair-1000.online"

    sort "${BATS_TMPDIR}/network_pair-1000.naive" | uniq > "${BATS_TMPDIR}/network_pair-1000.naive.sorted"
    sort "${BATS_TMPDIR}/network_pair-1000.online" | uniq > "${BATS_TMPDIR}/network_pair-1000.online.sorted"

    diff "${BATS_TMPDIR}/network_pair-1000.naive.sorted" "${BATS_TMPDIR}/network_pair-1000.online.sorted"
    [ $status -eq 0 ]
}

@test "Compare the result of Naive and NaiveFiltered" {
    cd "$PROJECT_ROOT"

    run cargo run --release -- -f "${EXAMPLE_DIR}/network_pair/network_pair.json" -i "${BATS_TMPDIR}/network_pair-1000.input" -m naive -o "${BATS_TMPDIR}/network_pair-1000.naive"
    run cargo run --release -- -f "${EXAMPLE_DIR}/network_pair/network_pair.json" -i "${BATS_TMPDIR}/network_pair-1000.input" -m naive-filtered -o "${BATS_TMPDIR}/network_pair-1000.naive_filtered"

    sort "${BATS_TMPDIR}/network_pair-1000.naive" | uniq > "${BATS_TMPDIR}/network_pair-1000.naive.sorted"
    sort "${BATS_TMPDIR}/network_pair-1000.naive_filtered" | uniq > "${BATS_TMPDIR}/network_pair-1000.naive_filtered.sorted"

    diff "${BATS_TMPDIR}/network_pair-1000.naive.sorted" "${BATS_TMPDIR}/network_pair-1000.naive_filtered.sorted"
    [ $status -eq 0 ]
}

@test "Compare the result of Naive and OnlineFiltered" {
    cd "$PROJECT_ROOT"

    run cargo run --release -- -f "${EXAMPLE_DIR}/network_pair/network_pair.json" -i "${BATS_TMPDIR}/network_pair-1000.input" -m naive -o "${BATS_TMPDIR}/network_pair-1000.naive"
    run cargo run --release -- -f "${EXAMPLE_DIR}/network_pair/network_pair.json" -i "${BATS_TMPDIR}/network_pair-1000.input" -m online-filtered -o "${BATS_TMPDIR}/network_pair-1000.online_filtered"

    sort "${BATS_TMPDIR}/network_pair-1000.naive" | uniq > "${BATS_TMPDIR}/network_pair-1000.naive.sorted"
    sort "${BATS_TMPDIR}/network_pair-1000.online_filtered" | uniq > "${BATS_TMPDIR}/network_pair-1000.online_filtered.sorted"

    diff "${BATS_TMPDIR}/network_pair-1000.naive.sorted" "${BATS_TMPDIR}/network_pair-1000.online_filtered.sorted"
    [ $status -eq 0 ]
}

@test "Compare the result of Naive and FJSFiltered" {
    cd "$PROJECT_ROOT"

    run cargo run --release -- -f "${EXAMPLE_DIR}/network_pair/network_pair.json" -i "${BATS_TMPDIR}/network_pair-1000.input" -m naive -o "${BATS_TMPDIR}/network_pair-1000.naive"
    run cargo run --release -- -f "${EXAMPLE_DIR}/network_pair/network_pair.json" -i "${BATS_TMPDIR}/network_pair-1000.input" -m fjs-filtered -o "${BATS_TMPDIR}/network_pair-1000.fjs_filtered"

    sort "${BATS_TMPDIR}/network_pair-1000.naive" | uniq > "${BATS_TMPDIR}/network_pair-1000.naive.sorted"
    sort "${BATS_TMPDIR}/network_pair-1000.fjs_filtered" | uniq > "${BATS_TMPDIR}/network_pair-1000.fjs_filtered.sorted"

    diff "${BATS_TMPDIR}/network_pair-1000.naive.sorted" "${BATS_TMPDIR}/network_pair-1000.fjs_filtered.sorted"
    [ $status -eq 0 ]
}
