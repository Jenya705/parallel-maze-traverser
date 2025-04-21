declare -a bfs_impls=("bfsstbs" "bfsmtabs" "bfsmtcsbs")
declare -a as_impls=("asmd" "asdpmd" "as2dbfs")
declare -a bfs2d_impls=("bfs2d")
declare -a unicode_impls=("u")

all_impls=("${bfs_impls[@]}" "${as_impls[@]}" "${bfs2d_impls[@]}" "${unicode_impls[@]}")

if [[ $1 == "bfs" ]]; then
    to_handle=("${bfs_impls[@]}")
elif [[ $1 == "as" ]]; then
    to_handle=("${as_impls[@]}")
elif [[ $1 == "2d" ]]; then
    to_handle=("${bfs2d_impls[@]}")
elif [[ $1 == "uni" ]]; then
    to_handle=("${unicode_impls[@]}")
else
    to_handle=("${all_impls[@]}")
fi

for j in $(seq 0 3); 
do
    for i in $(seq 0 9);
    do
        for impl in "${to_handle[@]}"
        do
            if [[ $impl == "u" ]]; then
                echo $impl $i
                cargo run --release -- -t 8 -u -p bfsmtcsbs labyrinthe$i.txt >> ./outputs/$impl\_$i\_u.txt
            else 
                echo $impl $i
                cargo run --release -- -t 4 -p $impl labyrinthe$i.txt >> ./outputs/$impl\_$i\_4.txt
                if [[ $impl == "bfsmt"* ]]; then
                    echo $impl $i 8
                    cargo run --release -- -t 8 -p $impl labyrinthe$i.txt >> ./outputs/$impl\_$i\_8.txt
                fi
                if [[ $impl == "as"* ]]; then
                    echo $impl $i m
                    cargo run --release -- -m -p $impl labyrinthe$i.txt >> ./outputs/$impl\_$i\_m.txt
                fi
                if [[ $impl == "bfsmtcsbs" || $impl == "as"* ]]; then
                    echo $impl $i w
                    cargo run --features written_count --release -- -p $impl labyrinthe$i.txt >> ./outputs/$impl\_$i\_w.txt
                fi
            fi
        done
    done
done