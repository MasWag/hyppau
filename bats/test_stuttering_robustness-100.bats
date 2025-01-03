#!/usr/bin/env bats

setup() {
    PROJECT_ROOT="${BATS_TEST_DIRNAME}/.."
    EXAMPLE_DIR="${PROJECT_ROOT}/examples"
    # Generate the input string
    seq 100 | "${EXAMPLE_DIR}/stuttering_robustness/gen_log.awk" -v ACTIONS='a,b' -v OUTPUTS='0,1' > "${BATS_TMPDIR}/stuttering_robustness_ab_01-100.input"
    # Generate the NFAH
    "${EXAMPLE_DIR}/stuttering_robustness/gen_stuttering_robustness.py" --inputs a b --outputs 0 1 > "${BATS_TMPDIR}/stuttering_robustness_ab_01.json"
}

@test "Compare the result of Naive and FJS" {
    cd "$PROJECT_ROOT"

    run cargo run --release -- -f "${BATS_TMPDIR}/stuttering_robustness_ab_01.json" -i "${BATS_TMPDIR}/stuttering_robustness_ab_01-100.input" -m naive -o "${BATS_TMPDIR}/stuttering_robustness_ab_01-100.naive"
    run cargo run --release -- -f "${BATS_TMPDIR}/stuttering_robustness_ab_01.json" -i "${BATS_TMPDIR}/stuttering_robustness_ab_01-100.input" -m fjs -o "${BATS_TMPDIR}/stuttering_robustness_ab_01-100.fjs"

    sort "${BATS_TMPDIR}/stuttering_robustness_ab_01-100.naive" | uniq > "${BATS_TMPDIR}/stuttering_robustness_ab_01-100.naive.sorted"
    sort "${BATS_TMPDIR}/stuttering_robustness_ab_01-100.fjs" | uniq > "${BATS_TMPDIR}/stuttering_robustness_ab_01-100.fjs.sorted"

    diff "${BATS_TMPDIR}/stuttering_robustness_ab_01-100.naive.sorted" "${BATS_TMPDIR}/stuttering_robustness_ab_01-100.fjs.sorted"
    [ $status -eq 0 ]
}

@test "Compare the result of Naive and Online" {
    cd "$PROJECT_ROOT"

    run cargo run --release -- -f "${BATS_TMPDIR}/stuttering_robustness_ab_01.json" -i "${BATS_TMPDIR}/stuttering_robustness_ab_01-100.input" -m naive -o "${BATS_TMPDIR}/stuttering_robustness_ab_01-100.naive"
    run cargo run --release -- -f "${BATS_TMPDIR}/stuttering_robustness_ab_01.json" -i "${BATS_TMPDIR}/stuttering_robustness_ab_01-100.input" -m online -o "${BATS_TMPDIR}/stuttering_robustness_ab_01-100.online"

    sort "${BATS_TMPDIR}/stuttering_robustness_ab_01-100.naive" | uniq > "${BATS_TMPDIR}/stuttering_robustness_ab_01-100.naive.sorted"
    sort "${BATS_TMPDIR}/stuttering_robustness_ab_01-100.online" | uniq > "${BATS_TMPDIR}/stuttering_robustness_ab_01-100.online.sorted"

    diff "${BATS_TMPDIR}/stuttering_robustness_ab_01-100.naive.sorted" "${BATS_TMPDIR}/stuttering_robustness_ab_01-100.online.sorted"
    [ $status -eq 0 ]
}

@test "Compare the result of Naive and NaiveFiltered" {
    cd "$PROJECT_ROOT"

    run cargo run --release -- -f "${BATS_TMPDIR}/stuttering_robustness_ab_01.json" -i "${BATS_TMPDIR}/stuttering_robustness_ab_01-100.input" -m naive -o "${BATS_TMPDIR}/stuttering_robustness_ab_01-100.naive"
    run cargo run --release -- -f "${BATS_TMPDIR}/stuttering_robustness_ab_01.json" -i "${BATS_TMPDIR}/stuttering_robustness_ab_01-100.input" -m naive-filtered -o "${BATS_TMPDIR}/stuttering_robustness_ab_01-100.naive_filtered"

    sort "${BATS_TMPDIR}/stuttering_robustness_ab_01-100.naive" | uniq > "${BATS_TMPDIR}/stuttering_robustness_ab_01-100.naive.sorted"
    sort "${BATS_TMPDIR}/stuttering_robustness_ab_01-100.naive_filtered" | uniq > "${BATS_TMPDIR}/stuttering_robustness_ab_01-100.naive_filtered.sorted"

    diff "${BATS_TMPDIR}/stuttering_robustness_ab_01-100.naive.sorted" "${BATS_TMPDIR}/stuttering_robustness_ab_01-100.naive_filtered.sorted"
    [ $status -eq 0 ]
}

@test "Compare the result of Naive and OnlineFiltered" {
    cd "$PROJECT_ROOT"

    run cargo run --release -- -f "${BATS_TMPDIR}/stuttering_robustness_ab_01.json" -i "${BATS_TMPDIR}/stuttering_robustness_ab_01-100.input" -m naive -o "${BATS_TMPDIR}/stuttering_robustness_ab_01-100.naive"
    run cargo run --release -- -f "${BATS_TMPDIR}/stuttering_robustness_ab_01.json" -i "${BATS_TMPDIR}/stuttering_robustness_ab_01-100.input" -m online-filtered -o "${BATS_TMPDIR}/stuttering_robustness_ab_01-100.online_filtered"

    sort "${BATS_TMPDIR}/stuttering_robustness_ab_01-100.naive" | uniq > "${BATS_TMPDIR}/stuttering_robustness_ab_01-100.naive.sorted"
    sort "${BATS_TMPDIR}/stuttering_robustness_ab_01-100.online_filtered" | uniq > "${BATS_TMPDIR}/stuttering_robustness_ab_01-100.online_filtered.sorted"

    diff "${BATS_TMPDIR}/stuttering_robustness_ab_01-100.naive.sorted" "${BATS_TMPDIR}/stuttering_robustness_ab_01-100.online_filtered.sorted"
    [ $status -eq 0 ]
}

@test "Compare the result of Naive and FJSFiltered" {
    cd "$PROJECT_ROOT"

    run cargo run --release -- -f "${BATS_TMPDIR}/stuttering_robustness_ab_01.json" -i "${BATS_TMPDIR}/stuttering_robustness_ab_01-100.input" -m naive -o "${BATS_TMPDIR}/stuttering_robustness_ab_01-100.naive"
    run cargo run --release -- -f "${BATS_TMPDIR}/stuttering_robustness_ab_01.json" -i "${BATS_TMPDIR}/stuttering_robustness_ab_01-100.input" -m fjs-filtered -o "${BATS_TMPDIR}/stuttering_robustness_ab_01-100.fjs_filtered"

    sort "${BATS_TMPDIR}/stuttering_robustness_ab_01-100.naive" | uniq > "${BATS_TMPDIR}/stuttering_robustness_ab_01-100.naive.sorted"
    sort "${BATS_TMPDIR}/stuttering_robustness_ab_01-100.fjs_filtered" | uniq > "${BATS_TMPDIR}/stuttering_robustness_ab_01-100.fjs_filtered.sorted"

    diff "${BATS_TMPDIR}/stuttering_robustness_ab_01-100.naive.sorted" "${BATS_TMPDIR}/stuttering_robustness_ab_01-100.fjs_filtered.sorted"
    [ $status -eq 0 ]
}
