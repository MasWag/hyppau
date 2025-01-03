#!/usr/bin/awk -f
# Name
#  gen_abcd.awk
#
# Description
#  Script to generate a random sequence of a, b, c and d.
#
# Example
#  seq 100 | ./gen_abcd.awk
#
# Author
#  Masaki Waga
#
# License
#  MIT License

{
    r = int(4 * rand())
}
r == 0 {
    print("a")
}
r == 1 {
    print("b")
}
r == 2 {
    print("c")
}
r == 3 {
    print("d")
}
