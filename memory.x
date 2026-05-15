/* Linker script for the STM32F103C8 */
MEMORY
{
  /* NOTE 1 K = 1 KiBi = 1024 bytes */
  FLASH : ORIGIN = 0x08000000, LENGTH = 63K
  /* Last 1 K page (0x0800_FC00) reserved for persistent settings */
  RAM : ORIGIN = 0x20000000, LENGTH = 20K
}

