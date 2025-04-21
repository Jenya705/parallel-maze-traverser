import os
import re
import sys

dir = "outputs"

times = {}
instructions = {}
writtens = {}
lens = {}
texts={}

for file in os.listdir(dir):
    with open(dir + "/" + file,encoding="utf8") as f:
        txt = f.read().split("\n")
        single_times = []
        single_instructions = []
        single_writtens=[]
        single_lens=[]
        single_text=""
        j=0
        for line in txt:
            if j < 4:
                single_text+=line+"\n"
            j+=1
            i = line.find("time elapsed:")
            if i!=-1:
                time = line[i+len("time elapsed:"):].strip()
                time_v = 0
                if time.endswith("ms"):
                    time_v = float(time[:-2]) * (10**3)
                elif time.endswith("µs"):
                    time_v = float(time[:-2])
                elif time.endswith("s"):
                    time_v = float(time[:-1]) * (10**6)
                else:
                    print("Failed")
                single_times.append(time_v)
            i = line.find("Instructions:")
            if i!=-1:
                instr = line[i+len("Instructions:"):].strip()
                single_instructions.append(int(instr))
            i = line.find("len:")
            if i!=-1:
                single_lens.append(int(line[i+len("len:"):].strip()))
            i=line.find("written:")
            if i!=-1:
                single_writtens.append(int(line[i+len("written:"):].strip()))
        avg = 0 if len(single_times)==0 else sum(single_times) / len(single_times)
        if avg > (10**6):
            avg_s = str(int(avg / (10**6) * 1000) / 1000.0) + "s"
        elif avg > (10**3):
            avg_s = str(int(avg / (10**3) * 1000) / 1000.0) + "ms"
        else:
            avg_s = str(int(avg * 1000) / 1000.0) + "$\\upmu\\text{s}$"
        times[file]=avg_s
        avg_i = -1 if len(single_instructions)==0 else sum(single_instructions)/len(single_instructions)
        instructions[file]=avg_i
        writtens[file]=-1 if len(single_writtens)==0 else sum(single_writtens)/len(single_writtens)
        lens[file]=-1 if len(single_lens)==0 else sum(single_lens)/len(single_lens)
        texts[file]=single_text

def put_newline_each(text, l):
    res = ""
    k = 0
    for c in text:
        if c == "\n":
            k=0
        if k == l:
            res+="\n"
            k=0
        k+=1
        res+=c
    while res.endswith("\n"):
        res=res[0:-1]
    return res

def splitting_commas(s):
    r = ""
    while s != "":
        s=s+" "
        r=","+s[-4:-1]+r
        s=s[0:-4]
    while r.startswith(","):
        r=r[1:]
    return r

if len(sys.argv)>1 and sys.argv[1]=="true":
    for i in range(0,10):
        file="u_i_u.txt".replace("i",str(i))
        print(r"\subsection{labyrinthe"+str(i)+"}")
        print(r"\begin{verbatim}")
        print(put_newline_each(texts[file],64))
        print(r"\end{verbatim}")
else:
    for i in range(0, 10):
        print(i, end="")
        for file in ["bfsstbs_i_4.txt", "bfsmtabs_i_4.txt", "bfsmtcsbs_i_4.txt", "bfsmtabs_i_8.txt", "bfsmtcsbs_i_8.txt"]:
            file=file.replace("i", str(i))
            print("&", times[file], end="")
        print("\\\\ \\hline")

    print()
    for i in range(0, 10):
        print(i, end="")
        for file in ["bfsstbs_i_4.txt", "asmd_i_4.txt", "asmd_i_m.txt", "asdpmd_i_4.txt", "asdpmd_i_m.txt", "as2dbfs_i_4.txt", "as2dbfs_i_m.txt"]:
            file=file.replace("i", str(i))
            print("&", times[file], end="")
        print("\\\\ \\hline")

    print()
    for i in range(0, 10):
        print(i, end="")
        for file in ["bfsstbs_i_4.txt", "asmd_i_4.txt", "asdpmd_i_4.txt", "as2dbfs_i_4.txt"]:
            file=file.replace("i",str(i))
            print("&",int(round(instructions[file])),end="")
        print("\\\\ \\hline")

    print()
    for i in range(0, 10):
        print(i,"&",splitting_commas(str(int(lens["bfsmtcsbs_"+str(i)+"_w.txt"]))),end="")
        for file in ["bfsmtcsbs_i_w.txt", "asmd_i_w.txt", "asdpmd_i_w.txt", "as2dbfs_i_w.txt"]:
            file=file.replace("i",str(i))
            val=int(round(writtens[file]))
            print("&",splitting_commas(str(val)),"&",int(round(val*10000/lens[file]))/100,end="")
        print("\\\\ \\hline")

    print()
    for i in range(0,10):
        print(i,end="")
        for file in ["bfsmtcsbs_i_4.txt"]:
            file=file.replace("i",str(i))
            print("&",int(instructions[file]),end="")
        for file in ["as2dbfs_i_m.txt","bfs2d_i_4.txt"]:
            file=file.replace("i",str(i))
            print("&",int(instructions[file]),"&",times[file],end="")
        print("\\\\ \\hline")