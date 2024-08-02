import sys
import os
import glob
import numpy as np
import pandas as pd
import matplotlib.pyplot as plt

def process_csv_files(folder_path):
    csv_files = glob.glob(os.path.join(folder_path, "*.csv"))

    averages = []
    stddevs = []
    file_names = []

    csv_files.sort()

    for file in csv_files:
        data = pd.read_csv(file, header=None, usecols=[0])
        
        data_ms = data[0] / 1000 # convert to ms
        
        avg = data_ms.mean()
        stddev = data_ms.std()
        
        # file_names.append(os.path.basename(file).replace('_', ' '))
        file_names.append(os.path.basename(file))
        averages.append(avg)
        stddevs.append(stddev)

    x_pos = np.arange(len(file_names))

    # Determine bar colors based on filename
    # colors = ['blue' if 'nofg' in file_name else 'red' for file_name in file_names]
    colors = ['red' if 'nofg' in file_name else 'blue' for file_name in file_names]
    
    fig, ax = plt.subplots()
    bars = ax.bar(x_pos, averages, yerr=stddevs, alpha=0.7, ecolor='black', capsize=10, color=colors)
    ax.set_ylabel('Time (ms)')
    ax.set_xticks(x_pos)
    ax.set_xticklabels(file_names, rotation=45, ha='right')
    ax.set_title('Latency of different FG tech')
    ax.yaxis.grid(True)

    plt.tight_layout()
    plt.savefig('input_latency.png')
    plt.show()

def main():
    path = "data"
    if len(sys.argv) > 1:
        path = sys.argv[1]
    print(f"Using csv files from: {path}")
    if os.path.isdir(path):
        process_csv_files(path)
    else:
        print("Folder doesn't exist")

if __name__ == "__main__":
    main()