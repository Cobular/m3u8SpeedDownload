import os
import requests
from urllib.parse import urljoin, urlparse
from concurrent.futures import Future, ThreadPoolExecutor, as_completed
from tqdm import tqdm
from typing import List, Optional
import subprocess
import argparse

def parse_arguments() -> argparse.Namespace:
    parser: argparse.ArgumentParser = argparse.ArgumentParser(description="Download and process M3U8 files.")
    parser.add_argument("url", type=str, help="URL of the M3U8 file to download")
    parser.add_argument("--output", type=str, default="output.mp4", help="Output file name")
    parser.add_argument("--compress", "-c", action="store_true", help="Enable compression")
    return parser.parse_args()

def download_ts_segment(ts_url: str, output_folder: str, session: requests.Session) -> None:
    try:
        # Extract the filename from the URL
        filename: str = os.path.basename(urlparse(ts_url).path)
        output_path: str = os.path.join(output_folder, filename)
        
        # Download the segment
        ts_content: bytes = session.get(ts_url).content
        
        # Save the segment to the specified output path
        with open(output_path, 'wb') as output_file:
            output_file.write(ts_content)
        # print(f"Downloaded {filename}")
    except Exception as e:
        print(f"Failed to download {ts_url}: {e}")

def download_m3u8(m3u8_url: str, output_folder: str) -> None:
    # Create a session
    session: requests.Session = requests.Session()
    
    # Get the m3u8 file content
    m3u8_content: str = session.get(m3u8_url).text
    
    # Ensure the output folder exists
    os.makedirs(output_folder, exist_ok=True)
    
    # Find all the .ts files
    lines: List[str] = m3u8_content.splitlines()
    ts_urls: List[str] = [urljoin(m3u8_url, line) for line in lines if line and not line.startswith("#")]

    # Download each .ts file in parallel with progress bar
    with ThreadPoolExecutor(max_workers=10) as executor:
        futures: List[Future[None]] = [executor.submit(download_ts_segment, ts_url, output_folder, session) for ts_url in ts_urls]
        
        with tqdm(total=len(futures), unit="segment") as progress_bar:
            for future in as_completed(futures):
                future.result()
                progress_bar.update(1)
    
    print(f"Downloaded all segments to the '{output_folder}' folder successfully.")

def create_file_list(output_folder: str, list_file_name: str = 'file_list.txt') -> None:
    ts_files: List[str] = sorted([f for f in os.listdir(output_folder) if f.endswith('.ts')])
    with open(list_file_name, 'w') as file_list:
        for ts_file in ts_files:
            file_list.write(f"file '{os.path.join(output_folder, ts_file)}'\n")
    
    print(f"Created {list_file_name} with {len(ts_files)} files listed.")

def execute_ffmpeg_command(input_file: str, output_file: str, compress: bool) -> None:
    command: List[str] = [
        "ffmpeg",
        "-f", "concat",
        "-safe", "0",
        "-i", input_file,
    ]
    
    if compress:
        command.extend([
            "-c:v", "libx264",
            "-crf", "23",
            "-preset", "medium",
            "-c:a", "aac",
            "-b:a", "128k",
        ])
    else:
        command.extend(["-c", "copy"])
    
    command.append(output_file)
    
    try:
        subprocess.run(command, check=True, capture_output=False, text=True)
        print(f"Successfully created {output_file}")
    except subprocess.CalledProcessError as e:
        print(f"Error executing ffmpeg command: {e}")
        print(f"ffmpeg stderr output: {e.stderr}")

if __name__ == "__main__":
    args: argparse.Namespace = parse_arguments()
    
    # Usage
    download_m3u8(args.url, 'output')
    create_file_list('output')
    
    # Execute the ffmpeg command
    execute_ffmpeg_command('file_list.txt', args.output, args.compress)
    
    if args.compress:
        print("Video compressed using libx264 and aac audio.")
        
    # Clean up the output folder
    import shutil
    shutil.rmtree('output')