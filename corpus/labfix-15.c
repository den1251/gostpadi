/* ЛБ2, задача 6: switch в цикле.
 * Дни месяца, выход по нулю. */
#include <stdio.h>

#define MONTHS 12

int main(void) {
    int month = 0;
    int days = 0;

    while (1) {
        printf("номер месяца (1..%d), 0 - выход: ", MONTHS);
        if (scanf("%d", &month) != 1) {
            printf("нужно число\n");
            while (getchar() != '\n') {
            }
            continue;
        }
        if (month == 0) {
            break;
        }
        if (month < 1 || month > MONTHS) {
            printf("месяца %d нет\n", month);
            continue;
        }
        switch (month) {
            case 2:
                days = 28;
                break;
            case 4:
            case 6:
            case 9:
            case 11:
                days = 30;
                break;
            default:
                days = 31;
                break;
        }
        printf("в месяце %d дней: %d\n", month, days);
    }
    return 0;
}
