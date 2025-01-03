#!/usr/bin/env bats

setup() {
    PROJECT_ROOT="${BATS_TEST_DIRNAME}/.."
    EXAMPLE_DIR="${PROJECT_ROOT}/examples"
    # Generate the input string
    seq 200 | "${EXAMPLE_DIR}/interference/gen_log.awk" -v ACTIONS='a,b' -v OUTPUTS='0,1' > "${BATS_TMPDIR}/interference_ab_01-200.input"
    # Generate the NFAH
    "${EXAMPLE_DIR}/interference/gen_interference.py" --actions a b --outputs 0 1 > "${BATS_TMPDIR}/interference_ab_01.json"
}

@test "Compare the result of Naive and FJS" {
    cd "$PROJECT_ROOT"

    run cargo run --release -- -f "${BATS_TMPDIR}/interference_ab_01.json" -i "${BATS_TMPDIR}/interference_ab_01-200.input" -m naive -o "${BATS_TMPDIR}/interference_ab_01-200.naive"
    run cargo run --release -- -f "${BATS_TMPDIR}/interference_ab_01.json" -i "${BATS_TMPDIR}/interference_ab_01-200.input" -m fjs -o "${BATS_TMPDIR}/interference_ab_01-200.fjs"

    sort "${BATS_TMPDIR}/interference_ab_01-200.naive" | uniq > "${BATS_TMPDIR}/interference_ab_01-200.naive.sorted"
    sort "${BATS_TMPDIR}/interference_ab_01-200.fjs" | uniq > "${BATS_TMPDIR}/interference_ab_01-200.fjs.sorted"

    diff "${BATS_TMPDIR}/interference_ab_01-200.naive.sorted" "${BATS_TMPDIR}/interference_ab_01-200.fjs.sorted"
    [ $status -eq 0 ]
}

@test "Compare the result of Naive and Online" {
    cd "$PROJECT_ROOT"

    run cargo run --release -- -f "${BATS_TMPDIR}/interference_ab_01.json" -i "${BATS_TMPDIR}/interference_ab_01-200.input" -m naive -o "${BATS_TMPDIR}/interference_ab_01-200.naive"
    run cargo run --release -- -f "${BATS_TMPDIR}/interference_ab_01.json" -i "${BATS_TMPDIR}/interference_ab_01-200.input" -m online -o "${BATS_TMPDIR}/interference_ab_01-200.online"

    sort "${BATS_TMPDIR}/interference_ab_01-200.naive" | uniq > "${BATS_TMPDIR}/interference_ab_01-200.naive.sorted"
    sort "${BATS_TMPDIR}/interference_ab_01-200.online" | uniq > "${BATS_TMPDIR}/interference_ab_01-200.online.sorted"

    diff "${BATS_TMPDIR}/interference_ab_01-200.naive.sorted" "${BATS_TMPDIR}/interference_ab_01-200.online.sorted"
    [ $status -eq 0 ]
}

@test "Compare the result of Naive and NaiveFiltered" {
    cd "$PROJECT_ROOT"

    run cargo run --release -- -f "${BATS_TMPDIR}/interference_ab_01.json" -i "${BATS_TMPDIR}/interference_ab_01-200.input" -m naive -o "${BATS_TMPDIR}/interference_ab_01-200.naive"
    run cargo run --release -- -f "${BATS_TMPDIR}/interference_ab_01.json" -i "${BATS_TMPDIR}/interference_ab_01-200.input" -m naive-filtered -o "${BATS_TMPDIR}/interference_ab_01-200.naive_filtered"

    sort "${BATS_TMPDIR}/interference_ab_01-200.naive" | uniq > "${BATS_TMPDIR}/interference_ab_01-200.naive.sorted"
    sort "${BATS_TMPDIR}/interference_ab_01-200.naive_filtered" | uniq > "${BATS_TMPDIR}/interference_ab_01-200.naive_filtered.sorted"

    diff "${BATS_TMPDIR}/interference_ab_01-200.naive.sorted" "${BATS_TMPDIR}/interference_ab_01-200.naive_filtered.sorted"
    [ $status -eq 0 ]
}

@test "Compare the result of Naive and OnlineFiltered" {
    cd "$PROJECT_ROOT"

    run cargo run --release -- -f "${BATS_TMPDIR}/interference_ab_01.json" -i "${BATS_TMPDIR}/interference_ab_01-200.input" -m naive -o "${BATS_TMPDIR}/interference_ab_01-200.naive"
    run cargo run --release -- -f "${BATS_TMPDIR}/interference_ab_01.json" -i "${BATS_TMPDIR}/interference_ab_01-200.input" -m online-filtered -o "${BATS_TMPDIR}/interference_ab_01-200.online_filtered"

    sort "${BATS_TMPDIR}/interference_ab_01-200.naive" | uniq > "${BATS_TMPDIR}/interference_ab_01-200.naive.sorted"
    sort "${BATS_TMPDIR}/interference_ab_01-200.online_filtered" | uniq > "${BATS_TMPDIR}/interference_ab_01-200.online_filtered.sorted"

    diff "${BATS_TMPDIR}/interference_ab_01-200.naive.sorted" "${BATS_TMPDIR}/interference_ab_01-200.online_filtered.sorted"
    [ $status -eq 0 ]
}

@test "Compare the result of Naive and FJSFiltered" {
    cd "$PROJECT_ROOT"

    run cargo run --release -- -f "${BATS_TMPDIR}/interference_ab_01.json" -i "${BATS_TMPDIR}/interference_ab_01-200.input" -m naive -o "${BATS_TMPDIR}/interference_ab_01-200.naive"
    run cargo run --release -- -f "${BATS_TMPDIR}/interference_ab_01.json" -i "${BATS_TMPDIR}/interference_ab_01-200.input" -m fjs-filtered -o "${BATS_TMPDIR}/interference_ab_01-200.fjs_filtered"

    sort "${BATS_TMPDIR}/interference_ab_01-200.naive" | uniq > "${BATS_TMPDIR}/interference_ab_01-200.naive.sorted"
    sort "${BATS_TMPDIR}/interference_ab_01-200.fjs_filtered" | uniq > "${BATS_TMPDIR}/interference_ab_01-200.fjs_filtered.sorted"

    diff "${BATS_TMPDIR}/interference_ab_01-200.naive.sorted" "${BATS_TMPDIR}/interference_ab_01-200.fjs_filtered.sorted"
    [ $status -eq 0 ]
}
