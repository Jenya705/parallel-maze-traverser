import os
import re
import sys

dir = "src"

files = [
    "main.rs",
    "delta_list.rs",
    "instructions.rs",
    "bfs.rs",
    "astar.rs",
    "graph.rs",
    "img.rs"
]

with open("srclatex.tex",mode="w",encoding="utf8") as wf:
    for file in files:
        with open(dir + "/" + file,encoding="utf8") as f:
            print(r"\subsection{"+file.replace("_","\\_")+"}",file=wf)
            print(r"\begin{lstlisting}",file=wf)
            print(f.read(),end="",file=wf)
            print(r"\end{lstlisting}",file=wf)